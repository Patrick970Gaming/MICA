//! Guest machine state, laid out so that compiled code can reach every field
//! with a single load or store off one base pointer.

pub const STACK_SIZE: usize = 512;

/// The guest register file and stack.
///
/// `repr(C)` is load-bearing: `codegen.rs` computes field offsets with
/// `offset_of!` and bakes them into the generated machine code, so the layout
/// must be stable and non-reordered.
#[repr(C)]
#[derive(Clone)]
pub struct CpuState {
    pub reg_a: u32,
    pub reg_b: u32,
    /// Accumulator and indirect-addressing index register. Every ALU op writes
    /// here, and the whole `*I` instruction family addresses through it.
    pub reg_c: u32,
    /// Flag register, written by `CMP`, read by the conditional jumps.
    pub reg_d: u32,
    /// Segment register. Loaded and stored but not yet interpreted by anything.
    pub reg_e: u32,
    /// Shared stack pointer: return addresses and `PSH`/`PLL` data live in the
    /// same array, so an unbalanced push corrupts the next `RET`.
    pub sp: u32,
    /// Set to 1 by compiled code when a store lands inside the code region.
    /// The dispatcher checks it after every block and flushes the cache.
    pub smc_dirty: u32,
    pub _pad: u32,
    pub stack: [u32; STACK_SIZE],
}

impl Default for CpuState {
    fn default() -> Self {
        Self {
            reg_a: 0,
            reg_b: 0,
            reg_c: 0,
            reg_d: 0,
            reg_e: 0,
            sp: 0,
            smc_dirty: 0,
            _pad: 0,
            stack: [0; STACK_SIZE],
        }
    }
}

impl PartialEq for CpuState {
    fn eq(&self, other: &Self) -> bool {
        self.reg_a == other.reg_a
            && self.reg_b == other.reg_b
            && self.reg_c == other.reg_c
            && self.reg_d == other.reg_d
            && self.reg_e == other.reg_e
            && self.sp == other.sp
            && self.stack[..] == other.stack[..]
    }
}

impl std::fmt::Debug for CpuState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "a={} b={} c={} d={} e={} sp={} stack_top={:?}",
            self.reg_a,
            self.reg_b,
            self.reg_c,
            self.reg_d,
            self.reg_e,
            self.sp,
            &self.stack[..self.sp.min(8) as usize]
        )
    }
}

/// Why a block or an interpreter step handed control back to the dispatcher.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    /// Ordinary end of block; `pc` is the next guest address to run.
    Continue = 0,
    /// `HAL` executed.
    Halt = 1,
    /// Undecodable opcode, or a fetch past the end of RAM.
    Invalid = 2,
    /// A store landed in the code region; the block cache must be flushed
    /// before `pc` is executed.
    SelfModified = 3,
}

impl Exit {
    pub fn from_u32(v: u32) -> Exit {
        match v {
            1 => Exit::Halt,
            2 => Exit::Invalid,
            3 => Exit::SelfModified,
            _ => Exit::Continue,
        }
    }
}

/// Compiled blocks return `(exit_reason << 32) | next_pc` packed into one i64,
/// which keeps their ABI to a single integer return register.
pub fn unpack_exit(packed: u64) -> (Exit, u32) {
    (Exit::from_u32((packed >> 32) as u32), packed as u32)
}

pub fn pack_exit(exit: Exit, pc: u32) -> u64 {
    ((exit as u64) << 32) | pc as u64
}

/// The whole guest: registers plus word-addressed RAM.
///
/// RAM length is forced to a power of two so that every guest address can be
/// folded into range with a single `AND` in compiled code instead of a bounds
/// check and a branch. This is a deliberate, documented deviation from
/// `MICA_inter_emu`, which panics on an out-of-range address.
pub struct Machine {
    pub cpu: CpuState,
    pub ram: Vec<u32>,
    pub addr_mask: u32,
    /// One past the last word of the loaded image. Stores below this may be
    /// self-modifying code and are guarded; stores at or above it are not.
    pub code_limit: u32,
}

impl Machine {
    pub fn new(ram_size: usize) -> Machine {
        assert!(
            ram_size.is_power_of_two(),
            "ram_size must be a power of two"
        );
        Machine {
            cpu: CpuState::default(),
            ram: vec![0; ram_size],
            addr_mask: (ram_size - 1) as u32,
            code_limit: 0,
        }
    }

    /// Load a big-endian 32-bit image at word address 0.
    pub fn load_image(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() % 4 != 0 {
            return Err(format!(
                "image length {} is not a multiple of 4",
                bytes.len()
            ));
        }
        let words = bytes.len() / 4;
        if words > self.ram.len() {
            return Err(format!(
                "image needs {words} words but ram_size is {}",
                self.ram.len()
            ));
        }
        for i in 0..words {
            self.ram[i] = u32::from_be_bytes([
                bytes[i * 4],
                bytes[i * 4 + 1],
                bytes[i * 4 + 2],
                bytes[i * 4 + 3],
            ]);
        }
        self.code_limit = words as u32;
        Ok(())
    }
}
