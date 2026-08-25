//! The dispatcher: the loop that decides, for each guest PC, whether to
//! interpret, compile, or jump into already-compiled code.
//!
//! Policy is deliberately simple and deliberately explicit, because this is
//! the knob that decides whether the JIT is faster than the interpreter on
//! real programs:
//!
//! * every PC has an execution counter;
//! * below `hot_threshold` the block is interpreted (cheap, no compile cost);
//! * at the threshold the block is discovered, translated and cached;
//! * afterwards the cached native function runs directly.
//!
//! A block returns the next guest PC, so control always comes back here. That
//! costs an indirect call and a hash lookup per block, and it is the first
//! thing to optimise away once correctness is established — see
//! `static_successors` in `block.rs` and the "block chaining" milestone in the
//! design doc.

use std::collections::HashMap;

use crate::block;
use crate::codegen::{BlockFn, Jit};
use crate::cpu::{unpack_exit, Exit, Machine};
use crate::interp;

pub struct EngineConfig {
    /// Executions of a block before it is compiled. 0 compiles on first sight.
    pub hot_threshold: u32,
    /// Safety valve so a runaway guest program cannot hang the emulator.
    pub max_steps: u64,
    pub trace: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        EngineConfig {
            hot_threshold: 8,
            max_steps: 100_000_000,
            trace: false,
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct Stats {
    pub guest_insns: u64,
    pub blocks_compiled: u64,
    pub native_entries: u64,
    pub interp_entries: u64,
    pub cache_flushes: u64,
}

#[derive(Clone, Copy)]
struct Compiled {
    func: BlockFn,
    /// Guest instructions in the block, so `Stats::guest_insns` stays
    /// comparable between the two tiers.
    len: u32,
}

pub struct Engine {
    jit: Jit,
    cache: HashMap<u32, Compiled>,
    counters: HashMap<u32, u32>,
    pub cfg: EngineConfig,
    pub stats: Stats,
}

impl Engine {
    pub fn new(cfg: EngineConfig) -> Result<Engine, String> {
        Ok(Engine {
            jit: Jit::new()?,
            cache: HashMap::new(),
            counters: HashMap::new(),
            cfg,
            stats: Stats::default(),
        })
    }

    /// Throw away every translation. Called when the guest writes to the code
    /// region. Coarse on purpose: MICA programs are small, and a correct flush
    /// is worth far more right now than a precise one. The design doc's
    /// milestone 5 replaces this with page-granular invalidation.
    pub fn flush(&mut self) {
        self.cache.clear();
        self.counters.clear();
        self.stats.cache_flushes += 1;
    }

    /// Run until halt, an invalid instruction, or the step budget runs out.
    pub fn run(&mut self, m: &mut Machine, entry: u32) -> Result<(Exit, u32), String> {
        let mut pc = entry;

        while self.stats.guest_insns < self.cfg.max_steps {
            let (exit, next) = self.step_block(m, pc)?;
            pc = next;
            match exit {
                Exit::Continue => {}
                Exit::SelfModified => {
                    m.cpu.smc_dirty = 0;
                    self.flush();
                }
                other => return Ok((other, pc)),
            }
        }
        Err(format!(
            "step budget of {} exhausted at pc {}",
            self.cfg.max_steps, pc
        ))
    }

    /// Run exactly one block's worth of guest work at `pc`, choosing the tier
    /// by the hotness policy. Public so the differential harness can drive the
    /// two tiers in lockstep at block granularity.
    pub fn step_block(&mut self, m: &mut Machine, pc: u32) -> Result<(Exit, u32), String> {
        if let Some(entry) = self.cache.get(&pc).copied() {
            return Ok(self.enter_native(m, entry));
        }

        let count = self.counters.entry(pc).or_insert(0);
        *count += 1;

        if *count > self.cfg.hot_threshold {
            let blk = block::discover(&m.ram, pc, m.code_limit);
            if self.cfg.trace {
                eprintln!(
                    "[jit] compiling block @{pc} ({} insns, terminator {})",
                    blk.len,
                    blk.terminator.map(|t| t.mnemonic()).unwrap_or("<cut>")
                );
            }
            let entry = Compiled {
                func: self.jit.compile(&blk, m.addr_mask, m.code_limit)?,
                len: blk.len as u32,
            };
            self.stats.blocks_compiled += 1;
            self.cache.insert(pc, entry);
            return Ok(self.enter_native(m, entry));
        }

        // Cold: interpret a whole block's worth so the counter advances at
        // block granularity rather than instruction granularity.
        self.stats.interp_entries += 1;
        let mut cursor = pc;
        loop {
            let insn = crate::isa::decode(&m.ram, cursor);
            let terminates = insn.flow != crate::isa::Flow::Linear;
            let (exit, next) = interp::step(m, cursor);
            self.stats.guest_insns += 1;
            cursor = next;
            match exit {
                Exit::Continue if !terminates => continue,
                other => return Ok((other, cursor)),
            }
        }
    }

    fn enter_native(&mut self, m: &mut Machine, entry: Compiled) -> (Exit, u32) {
        self.stats.native_entries += 1;
        self.stats.guest_insns += entry.len as u64;
        let f = entry.func;
        let cpu = &mut m.cpu as *mut _;
        let ram = m.ram.as_mut_ptr();
        // SAFETY: `f` was produced by `Jit::compile` with exactly this ABI, and
        // both pointers outlive the call. Every guest address the block forms
        // is masked into range at translation time, so the block cannot access
        // memory outside `m.ram` or outside `CpuState`.
        let packed = unsafe { f(cpu, ram) };
        unpack_exit(packed)
    }
}
