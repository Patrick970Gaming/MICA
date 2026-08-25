//! Cranelift translation of one MICA basic block into one native function.
//!
//! ABI of a compiled block:
//!
//! ```text
//! extern "C" fn(cpu: *mut CpuState, ram: *mut u32) -> u64
//! ```
//!
//! The return value is `(exit_reason << 32) | next_pc`, so a block hands
//! control back through a single integer register with no memory traffic.
//!
//! The important optimisation is not any Cranelift pass — it is that guest
//! registers become SSA values for the lifetime of the block. MICA is an
//! accumulator machine, so a straight-line run of `LDA/LDB/ADD/STC` touches
//! reg_a, reg_b and reg_c constantly; an interpreter turns each of those into
//! a memory access. Here they are loaded once at block entry, live in host
//! registers, and are written back once per exit edge. Cranelift's constant
//! folding and dead-store elimination then run on top of that for free.

use cranelift_codegen::ir::{types, AbiParam, InstBuilder, MemFlagsData, Signature, Value};
use cranelift_codegen::isa::CallConv;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::Context;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};
use std::mem::offset_of;

use crate::block::Block;
use crate::cpu::{CpuState, Exit, STACK_SIZE};
use crate::isa::{self, Flow, Insn};

pub type BlockFn = unsafe extern "C" fn(*mut CpuState, *mut u32) -> u64;

pub struct Jit {
    module: JITModule,
    frontend_cfg: cranelift_codegen::isa::TargetFrontendConfig,
    ctx: Context,
    fn_ctx: FunctionBuilderContext,
    sig: Signature,
    seq: usize,
}

impl Jit {
    pub fn new() -> Result<Jit, String> {
        let mut flags = settings::builder();
        flags.set("use_colocated_libcalls", "false").map_err(e)?;
        flags.set("is_pic", "false").map_err(e)?;
        flags.set("opt_level", "speed").map_err(e)?;

        let isa_builder = cranelift_native::builder().map_err(|m| m.to_string())?;
        let isa = isa_builder
            .finish(settings::Flags::new(flags))
            .map_err(|m| m.to_string())?;

        let call_conv = isa.default_call_conv();
        let frontend_cfg = isa.frontend_config();
        let module = JITModule::new(JITBuilder::with_isa(
            isa,
            cranelift_module::default_libcall_names(),
        ));

        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(types::I64)); // *mut CpuState
        sig.params.push(AbiParam::new(types::I64)); // *mut u32
        sig.returns.push(AbiParam::new(types::I64)); // packed (exit, pc)

        Ok(Jit {
            ctx: module.make_context(),
            module,
            frontend_cfg,
            fn_ctx: FunctionBuilderContext::new(),
            sig,
            seq: 0,
        })
    }

    pub fn call_conv(&self) -> CallConv {
        self.sig.call_conv
    }

    /// Compile `block` and return a pointer to the native code.
    pub fn compile(
        &mut self,
        block: &Block,
        addr_mask: u32,
        code_limit: u32,
    ) -> Result<BlockFn, String> {
        self.module.clear_context(&mut self.ctx);
        self.ctx.func.signature = self.sig.clone();

        {
            let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.fn_ctx);
            let entry = builder.create_block();
            builder.append_block_params_for_function_params(entry);
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            let cpu = builder.block_params(entry)[0];
            let ram = builder.block_params(entry)[1];

            let mut tx = Translator {
                b: builder,
                cpu,
                ram,
                addr_mask,
                code_limit,
                regs: Regs::default(),
                dyn_exit: None,
            };
            tx.load_regs();
            for insn in &block.body {
                tx.linear(insn);
            }
            tx.terminate(block);
            tx.b.finalize(self.frontend_cfg);
        }

        self.seq += 1;
        let name = format!("mica_block_{}_{:08x}", self.seq, block.start);
        let id = self
            .module
            .declare_function(&name, Linkage::Export, &self.sig)
            .map_err(|m| m.to_string())?;
        self.module
            .define_function(id, &mut self.ctx)
            .map_err(|m| m.to_string())?;
        self.module
            .finalize_definitions()
            .map_err(|m| m.to_string())?;

        let ptr = self.module.get_finalized_function(id);
        Ok(unsafe { std::mem::transmute::<*const u8, BlockFn>(ptr) })
    }

    /// Dump the Cranelift IR of the most recently compiled block. Handy when a
    /// differential test fails and you want to see what was emitted.
    pub fn last_ir(&self) -> String {
        self.ctx.func.display().to_string()
    }
}

fn e<T: std::fmt::Display>(msg: T) -> String {
    msg.to_string()
}

#[derive(Default, Clone, Copy)]
struct Regs {
    a: Option<Value>,
    b: Option<Value>,
    c: Option<Value>,
    d: Option<Value>,
    e: Option<Value>,
    sp: Option<Value>,
}

struct Translator<'a> {
    b: FunctionBuilder<'a>,
    cpu: Value,
    ram: Value,
    addr_mask: u32,
    code_limit: u32,
    regs: Regs,
    /// Set when a store might have hit the code region: the block's exit code
    /// becomes a runtime value instead of a constant, which avoids a branch.
    dyn_exit: Option<Value>,
}

const F: MemFlagsData = MemFlagsData::trusted();

impl<'a> Translator<'a> {
    fn load_regs(&mut self) {
        self.regs.a = Some(self.field(offset_of!(CpuState, reg_a)));
        self.regs.b = Some(self.field(offset_of!(CpuState, reg_b)));
        self.regs.c = Some(self.field(offset_of!(CpuState, reg_c)));
        self.regs.d = Some(self.field(offset_of!(CpuState, reg_d)));
        self.regs.e = Some(self.field(offset_of!(CpuState, reg_e)));
        self.regs.sp = Some(self.field(offset_of!(CpuState, sp)));
    }

    fn field(&mut self, offset: usize) -> Value {
        self.b.ins().load(types::I32, F, self.cpu, offset as i32)
    }

    fn a(&self) -> Value {
        self.regs.a.unwrap()
    }
    fn rb(&self) -> Value {
        self.regs.b.unwrap()
    }
    fn c(&self) -> Value {
        self.regs.c.unwrap()
    }
    fn d(&self) -> Value {
        self.regs.d.unwrap()
    }
    fn sp(&self) -> Value {
        self.regs.sp.unwrap()
    }

    /// Address of guest word `addr` inside the RAM allocation. The power-of-two
    /// RAM size means bounds safety costs one `AND`, no branch.
    fn word_ptr(&mut self, addr: Value) -> Value {
        let masked = self.b.ins().band_imm_u(addr, self.addr_mask as i64);
        let wide = self.b.ins().uextend(types::I64, masked);
        let byte_off = self.b.ins().ishl_imm_u(wide, 2);
        self.b.ins().iadd(self.ram, byte_off)
    }

    fn ram_load(&mut self, addr: Value) -> Value {
        let p = self.word_ptr(addr);
        self.b.ins().load(types::I32, F, p, 0)
    }

    fn ram_store(&mut self, addr: Value, value: Value) {
        let p = self.word_ptr(addr);
        self.b.ins().store(F, value, p, 0);
    }

    fn stack_ptr(&mut self, index: Value) -> Value {
        let masked = self.b.ins().band_imm_u(index, (STACK_SIZE - 1) as i64);
        let wide = self.b.ins().uextend(types::I64, masked);
        let byte_off = self.b.ins().ishl_imm_u(wide, 2);
        let base = self
            .b
            .ins()
            .iadd_imm_u(self.cpu, offset_of!(CpuState, stack) as i64);
        self.b.ins().iadd(base, byte_off)
    }

    fn push(&mut self, value: Value) {
        let sp = self.sp();
        let p = self.stack_ptr(sp);
        self.b.ins().store(F, value, p, 0);
        self.regs.sp = Some(self.b.ins().iadd_imm_u(sp, 1));
    }

    fn pop(&mut self) -> Value {
        let old = self.sp();
        let sp = self.b.ins().iadd_imm_s(old, -1);
        self.regs.sp = Some(sp);
        let p = self.stack_ptr(sp);
        self.b.ins().load(types::I32, F, p, 0)
    }

    fn fbin(&mut self, opcode: u32, a: Value, b: Value) -> Value {
        let fa = self.b.ins().bitcast(types::F32, F, a);
        let fb = self.b.ins().bitcast(types::F32, F, b);
        let r = match opcode {
            21 => self.b.ins().fadd(fa, fb),
            22 => self.b.ins().fsub(fa, fb),
            23 => self.b.ins().fmul(fa, fb),
            24 => self.b.ins().fdiv(fa, fb),
            _ => unreachable!(),
        };
        self.b.ins().bitcast(types::I32, F, r)
    }

    /// Translate one non-terminator instruction.
    fn linear(&mut self, insn: &Insn) {
        let op = insn.opcode;
        let operand = insn.operand;

        match op {
            0 => {}

            // Direct loads: the operand is the address of the data word.
            1 | 3 | 5 | 6 | 7 => {
                let addr = self.b.ins().iconst(types::I32, operand as i64);
                let v = self.ram_load(addr);
                match op {
                    1 => self.regs.a = Some(v),
                    3 => self.regs.b = Some(v),
                    5 => self.regs.c = Some(v),
                    6 => self.regs.d = Some(v),
                    _ => self.regs.e = Some(v),
                }
            }

            // Indirect loads through reg_c.
            2 | 4 => {
                let addr = self.c();
                let v = self.ram_load(addr);
                if op == 2 {
                    self.regs.a = Some(v);
                } else {
                    self.regs.b = Some(v);
                }
            }

            // Stores.
            8 | 10 | 12 | 13 | 14 | 9 | 11 => {
                let (value, addr) = match op {
                    8 => (self.a(), self.const_addr(operand)),
                    9 => (self.a(), self.c()),
                    10 => (self.rb(), self.const_addr(operand)),
                    11 => (self.rb(), self.c()),
                    12 => (self.c(), self.const_addr(operand)),
                    13 => (self.d(), self.const_addr(operand)),
                    _ => (self.regs.e.unwrap(), self.const_addr(operand)),
                };
                self.ram_store(addr, value);
                self.guard_smc(op, operand, addr);
            }

            15 => {
                let v = self.a();
                self.push(v);
            }
            16 => {
                let v = self.pop();
                self.regs.a = Some(v);
            }

            // Integer ALU. All wrapping, matching interp::alu.
            17 | 18 | 19 | 42 | 43 | 45 => {
                let (x, y) = (self.a(), self.rb());
                let r = match op {
                    17 => self.b.ins().iadd(x, y),
                    18 => self.b.ins().isub(x, y),
                    19 => self.b.ins().imul(x, y),
                    42 => self.b.ins().band(x, y),
                    43 => self.b.ins().bor(x, y),
                    _ => self.b.ins().bxor(x, y),
                };
                self.regs.c = Some(r);
            }

            // DIV, with division by zero defined as producing 0. Emitted
            // branchlessly: divide by a substituted 1, then select the result.
            20 => {
                let (x, y) = (self.a(), self.rb());
                let is_zero =
                    self.b
                        .ins()
                        .icmp_imm_u(cranelift_codegen::ir::condcodes::IntCC::Equal, y, 0);
                let one = self.b.ins().iconst(types::I32, 1);
                let safe = self.b.ins().select(is_zero, one, y);
                let q = self.b.ins().udiv(x, safe);
                let zero = self.b.ins().iconst(types::I32, 0);
                self.regs.c = Some(self.b.ins().select(is_zero, zero, q));
            }

            21..=24 => {
                let (x, y) = (self.a(), self.rb());
                let r = self.fbin(op, x, y);
                self.regs.c = Some(r);
            }

            39 => {
                let r = self.compare();
                self.regs.d = Some(r);
            }

            40 => {
                let v = self.a();
                self.regs.c = Some(self.b.ins().ushr_imm_u(v, 1));
            }
            41 => {
                let v = self.a();
                self.regs.c = Some(self.b.ins().ishl_imm_u(v, 1));
            }
            44 => {
                let v = self.a();
                self.regs.c = Some(self.b.ins().bnot(v));
            }
            46 => {
                let v = self.a();
                self.regs.c = Some(self.b.ins().ineg(v));
            }

            _ => {}
        }
    }

    fn const_addr(&mut self, operand: u32) -> Value {
        self.b.ins().iconst(types::I32, operand as i64)
    }

    /// `CMP`, expressed branchlessly as five predicates OR'd into a flag word.
    fn compare(&mut self) -> Value {
        use cranelift_codegen::ir::condcodes::IntCC;
        let (x, y) = (self.a(), self.rb());

        let eq = self.b.ins().icmp(IntCC::Equal, x, y);
        let sgt = self.b.ins().icmp(IntCC::SignedGreaterThan, x, y);
        let ugt = self.b.ins().icmp(IntCC::UnsignedGreaterThan, x, y);

        let eq32 = self.b.ins().uextend(types::I32, eq);
        let sgt32 = self.b.ins().uextend(types::I32, sgt);
        let ugt32 = self.b.ins().uextend(types::I32, ugt);

        // EQ = eq; GT = !eq &  sgt; LT = !eq & !sgt
        //           GTU = !eq &  ugt; LTU = !eq & !ugt
        let neq32 = self.b.ins().bxor_imm_u(eq32, 1);

        let gt = self.b.ins().band(neq32, sgt32);
        let nsgt = self.b.ins().bxor_imm_u(sgt32, 1);
        let lt = self.b.ins().band(neq32, nsgt);
        let gtu = self.b.ins().band(neq32, ugt32);
        let nugt = self.b.ins().bxor_imm_u(ugt32, 1);
        let ltu = self.b.ins().band(neq32, nugt);

        let f_eq = self.b.ins().imul_imm_u(eq32, isa::FLAG_EQ as i64);
        let f_gt = self.b.ins().imul_imm_u(gt, isa::FLAG_GT as i64);
        let f_lt = self.b.ins().imul_imm_u(lt, isa::FLAG_LT as i64);
        let f_gtu = self.b.ins().imul_imm_u(gtu, isa::FLAG_GTU as i64);
        let f_ltu = self.b.ins().imul_imm_u(ltu, isa::FLAG_LTU as i64);

        let mut acc = self.b.ins().bor(f_eq, f_gt);
        acc = self.b.ins().bor(acc, f_lt);
        acc = self.b.ins().bor(acc, f_gtu);
        self.b.ins().bor(acc, f_ltu)
    }

    /// After a store, decide whether the block must report self-modification.
    ///
    /// A store to a compile-time-constant address at or above `code_limit`
    /// cannot touch code, so it costs nothing. Anything else contributes a
    /// runtime exit code, which the block returns instead of `Continue`.
    fn guard_smc(&mut self, op: u32, operand: u32, addr: Value) {
        use cranelift_codegen::ir::condcodes::IntCC;
        if !isa::is_indirect_store(op) && operand >= self.code_limit {
            return;
        }
        let masked = self.b.ins().band_imm_u(addr, self.addr_mask as i64);
        let hits_code =
            self.b
                .ins()
                .icmp_imm_u(IntCC::UnsignedLessThan, masked, self.code_limit as i64);
        let dirty = self.b.ins().uextend(types::I32, hits_code);
        // Record it for the dispatcher even on the path where we keep going.
        self.b
            .ins()
            .store(F, dirty, self.cpu, offset_of!(CpuState, smc_dirty) as i32);
        let code = self.b.ins().imul_imm_u(dirty, Exit::SelfModified as i64);
        self.dyn_exit = Some(match self.dyn_exit {
            Some(prev) => self.b.ins().bor(prev, code),
            None => code,
        });
    }

    fn spill(&mut self) {
        let pairs = [
            (self.a(), offset_of!(CpuState, reg_a)),
            (self.rb(), offset_of!(CpuState, reg_b)),
            (self.c(), offset_of!(CpuState, reg_c)),
            (self.d(), offset_of!(CpuState, reg_d)),
            (self.regs.e.unwrap(), offset_of!(CpuState, reg_e)),
            (self.sp(), offset_of!(CpuState, sp)),
        ];
        for (v, off) in pairs {
            self.b.ins().store(F, v, self.cpu, off as i32);
        }
    }

    /// Emit the register write-back and the packed return value.
    fn exit(&mut self, exit: Exit, pc: Value) {
        self.spill();
        let pc64 = self.b.ins().uextend(types::I64, pc);
        let packed = match (exit, self.dyn_exit) {
            // A constant exit reason folds into an OR with an immediate.
            (Exit::Continue, None) => pc64,
            (_, None) => self.b.ins().bor_imm_u(pc64, (exit as i64) << 32),
            (Exit::Continue, Some(dynamic)) => {
                let wide = self.b.ins().uextend(types::I64, dynamic);
                let shifted = self.b.ins().ishl_imm_u(wide, 32);
                self.b.ins().bor(pc64, shifted)
            }
            (_, Some(_)) => self.b.ins().bor_imm_u(pc64, (exit as i64) << 32),
        };
        self.b.ins().return_(&[packed]);
    }

    fn exit_const(&mut self, exit: Exit, pc: u32) {
        let v = self.b.ins().iconst(types::I32, pc as i64);
        self.exit(exit, v);
    }

    fn terminate(&mut self, block: &Block) {
        let term = match block.terminator {
            Some(t) => t,
            // Block was cut for length or by a self-modifying store.
            None => {
                self.exit_const(Exit::Continue, block.fallthrough);
                return;
            }
        };

        match term.flow {
            Flow::Linear => self.exit_const(Exit::Continue, block.fallthrough),

            Flow::Halt => self.exit_const(Exit::Halt, term.addr),
            Flow::Invalid => self.exit_const(Exit::Invalid, term.addr),

            Flow::Call { target } => {
                let ret_addr = self.b.ins().iconst(types::I32, term.addr as i64);
                self.push(ret_addr);
                self.exit_const(Exit::Continue, target);
            }

            Flow::CallIndirect => {
                // 1-word call, pre-biased so RET's fixed +2 lands correctly.
                let biased = term.addr.wrapping_sub(isa::RET_BIAS - 1);
                let ret_addr = self.b.ins().iconst(types::I32, biased as i64);
                self.push(ret_addr);
                let target = self.c();
                self.exit(Exit::Continue, target);
            }

            Flow::Ret => {
                let saved = self.pop();
                let resume = self.b.ins().iadd_imm_u(saved, isa::RET_BIAS as i64);
                self.exit(Exit::Continue, resume);
            }

            Flow::Branch {
                mask,
                taken_when_set,
                target,
            } => {
                let cond = self.branch_cond(mask, taken_when_set);
                let taken = self.b.create_block();
                let fallthrough = self.b.create_block();
                self.b.ins().brif(cond, taken, &[], fallthrough, &[]);
                self.b.seal_block(taken);
                self.b.seal_block(fallthrough);

                let saved = self.regs;
                self.b.switch_to_block(taken);
                self.exit_const(Exit::Continue, target);

                self.regs = saved;
                self.b.switch_to_block(fallthrough);
                self.exit_const(Exit::Continue, block.fallthrough);
            }

            Flow::BranchIndirect {
                mask,
                taken_when_set,
            } => {
                let cond = self.branch_cond(mask, taken_when_set);
                let taken = self.b.create_block();
                let fallthrough = self.b.create_block();
                self.b.ins().brif(cond, taken, &[], fallthrough, &[]);
                self.b.seal_block(taken);
                self.b.seal_block(fallthrough);

                let saved = self.regs;
                self.b.switch_to_block(taken);
                let target = self.c();
                self.exit(Exit::Continue, target);

                self.regs = saved;
                self.b.switch_to_block(fallthrough);
                self.exit_const(Exit::Continue, block.fallthrough);
            }
        }
    }

    fn branch_cond(&mut self, mask: u32, taken_when_set: bool) -> Value {
        use cranelift_codegen::ir::condcodes::IntCC;
        let d = self.d();
        let bits = self.b.ins().band_imm_u(d, mask as i64);
        let cc = if taken_when_set {
            IntCC::NotEqual
        } else {
            IntCC::Equal
        };
        self.b.ins().icmp_imm_u(cc, bits, 0)
    }
}
