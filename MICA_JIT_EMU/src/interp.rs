//! Tier-0 interpreter.
//!
//! It has two jobs. It runs cold code, so the engine never pays compile cost
//! for a block that executes once. And it is the oracle the differential
//! harness checks the JIT against, which is why it lives in this crate rather
//! than being borrowed from `MICA_inter_emu`: it decodes through `isa.rs` and
//! implements exactly the semantics `codegen.rs` emits, including the three
//! places where those semantics are deliberately tightened relative to
//! `MICA_inter_emu` (wrapping arithmetic, masked addresses, defined
//! division by zero).

use crate::cpu::{Exit, Machine, STACK_SIZE};
use crate::isa::{self, Flow};

/// Wrapping is the defined behaviour: `MICA_inter_emu` uses plain `+`/`-`/`*`,
/// which panics on overflow in a debug build and wraps in release. Cranelift's
/// `iadd` always wraps, so the ISA has to pick one — it picks wrapping.
#[inline]
fn alu(opcode: u32, a: u32, b: u32) -> u32 {
    match opcode {
        17 => a.wrapping_add(b),
        18 => a.wrapping_sub(b),
        19 => a.wrapping_mul(b),
        // Division by zero yields 0 rather than trapping. Undefined behaviour
        // in an ISA is a liability for a recompiler: the JIT would have to
        // emit a guard branch on every DIV to reproduce a panic faithfully.
        20 => {
            if b == 0 {
                0
            } else {
                a / b
            }
        }
        21 => (f32::from_bits(a) + f32::from_bits(b)).to_bits(),
        22 => (f32::from_bits(a) - f32::from_bits(b)).to_bits(),
        23 => (f32::from_bits(a) * f32::from_bits(b)).to_bits(),
        24 => (f32::from_bits(a) / f32::from_bits(b)).to_bits(),
        42 => a & b,
        43 => a | b,
        45 => a ^ b,
        _ => unreachable!("not an ALU opcode: {opcode}"),
    }
}

pub fn compare(a: u32, b: u32) -> u32 {
    let mut flags = 0;
    if a == b {
        flags |= isa::FLAG_EQ;
    } else {
        if (a as i32) > (b as i32) {
            flags |= isa::FLAG_GT;
        } else {
            flags |= isa::FLAG_LT;
        }
        if a > b {
            flags |= isa::FLAG_GTU;
        } else {
            flags |= isa::FLAG_LTU;
        }
    }
    flags
}

/// Execute one instruction at `pc`. Returns the exit reason and the next PC.
pub fn step(m: &mut Machine, pc: u32) -> (Exit, u32) {
    let insn = isa::decode(&m.ram, pc);
    let next = pc.wrapping_add(insn.len);
    let mask = m.addr_mask;
    let ea = |addr: u32| (addr & mask) as usize;

    macro_rules! load_direct {
        ($reg:ident) => {{
            m.cpu.$reg = m.ram[ea(insn.operand)];
            return (Exit::Continue, next);
        }};
    }
    macro_rules! store_to {
        ($value:expr, $addr:expr) => {{
            let addr = $addr;
            m.ram[ea(addr)] = $value;
            if (addr & mask) < m.code_limit {
                m.cpu.smc_dirty = 1;
                return (Exit::SelfModified, next);
            }
            return (Exit::Continue, next);
        }};
    }

    match insn.opcode {
        0 => {}
        1 => load_direct!(reg_a),
        2 => {
            m.cpu.reg_a = m.ram[ea(m.cpu.reg_c)];
        }
        3 => load_direct!(reg_b),
        4 => {
            m.cpu.reg_b = m.ram[ea(m.cpu.reg_c)];
        }
        5 => load_direct!(reg_c),
        6 => load_direct!(reg_d),
        7 => load_direct!(reg_e),
        8 => store_to!(m.cpu.reg_a, insn.operand),
        9 => store_to!(m.cpu.reg_a, m.cpu.reg_c),
        10 => store_to!(m.cpu.reg_b, insn.operand),
        11 => store_to!(m.cpu.reg_b, m.cpu.reg_c),
        12 => store_to!(m.cpu.reg_c, insn.operand),
        13 => store_to!(m.cpu.reg_d, insn.operand),
        14 => store_to!(m.cpu.reg_e, insn.operand),
        15 => {
            let sp = m.cpu.sp as usize % STACK_SIZE;
            m.cpu.stack[sp] = m.cpu.reg_a;
            m.cpu.sp = m.cpu.sp.wrapping_add(1);
        }
        16 => {
            m.cpu.sp = m.cpu.sp.wrapping_sub(1);
            m.cpu.reg_a = m.cpu.stack[m.cpu.sp as usize % STACK_SIZE];
        }
        17..=24 | 42 | 43 | 45 => {
            m.cpu.reg_c = alu(insn.opcode, m.cpu.reg_a, m.cpu.reg_b);
        }
        39 => m.cpu.reg_d = compare(m.cpu.reg_a, m.cpu.reg_b),
        40 => m.cpu.reg_c = m.cpu.reg_a >> 1,
        41 => m.cpu.reg_c = m.cpu.reg_a << 1,
        44 => m.cpu.reg_c = !m.cpu.reg_a,
        46 => m.cpu.reg_c = m.cpu.reg_a.wrapping_neg(),
        _ => {}
    }

    match insn.flow {
        Flow::Linear => (Exit::Continue, next),
        Flow::Call { target } => {
            push(m, pc);
            (Exit::Continue, target)
        }
        Flow::CallIndirect => {
            // A 1-word call has to pre-compensate for RET's fixed +2 bias.
            push(m, pc.wrapping_sub(isa::RET_BIAS - 1));
            (Exit::Continue, m.cpu.reg_c)
        }
        Flow::Branch {
            mask: fmask,
            taken_when_set,
            target,
        } => {
            if taken(m.cpu.reg_d, fmask, taken_when_set) {
                (Exit::Continue, target)
            } else {
                (Exit::Continue, next)
            }
        }
        Flow::BranchIndirect {
            mask: fmask,
            taken_when_set,
        } => {
            if taken(m.cpu.reg_d, fmask, taken_when_set) {
                (Exit::Continue, m.cpu.reg_c)
            } else {
                (Exit::Continue, next)
            }
        }
        Flow::Ret => {
            m.cpu.sp = m.cpu.sp.wrapping_sub(1);
            let saved = m.cpu.stack[m.cpu.sp as usize % STACK_SIZE];
            (Exit::Continue, saved.wrapping_add(isa::RET_BIAS))
        }
        Flow::Halt => (Exit::Halt, pc),
        Flow::Invalid => (Exit::Invalid, pc),
    }
}

#[inline]
fn taken(flags: u32, mask: u32, when_set: bool) -> bool {
    (flags & mask != 0) == when_set
}

fn push(m: &mut Machine, value: u32) {
    let sp = m.cpu.sp as usize % STACK_SIZE;
    m.cpu.stack[sp] = value;
    m.cpu.sp = m.cpu.sp.wrapping_add(1);
}

/// Run to completion with no JIT involvement. Used by the differential harness
/// and by `--interp-only`.
pub fn run(m: &mut Machine, mut pc: u32, max_steps: u64) -> (Exit, u32, u64) {
    for executed in 0..max_steps {
        let (exit, next) = step(m, pc);
        pc = next;
        match exit {
            Exit::Continue | Exit::SelfModified => {}
            other => return (other, pc, executed + 1),
        }
    }
    (Exit::Continue, pc, max_steps)
}
