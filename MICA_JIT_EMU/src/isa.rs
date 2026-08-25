//! The single authoritative description of the MICA instruction set.
//!
//! Both the tier-0 interpreter (`interp.rs`) and the Cranelift translator
//! (`codegen.rs`) decode through this module, so the two tiers cannot drift
//! apart in opcode numbering, operand shape, or instruction length. That
//! property is what makes the differential harness meaningful: when the two
//! tiers disagree it is a translation bug, never a decoding bug.

/// Opcode numbering of the `"full"` instruction set standard, matching
/// `INSTRUCTION_SET_FULL` in `MICA_inter_emu/src/main.rs` and
/// `instruction_set_full` in `Assembler/assembler.py`.
pub const MNEMONICS: &[&str] = &[
    "NOP", "LDA", "LDAI", "LDB", "LDBI", "LDC", "LDD", "LDE", "STA", "STAI", "STB", "STBI", "STC",
    "STD", "STE", "PSH", "PLL", "ADD", "SUB", "MUL", "DIV", "FADD", "FSUB", "FMUL", "FDIV", "JMP",
    "JMPE", "JMPN", "JMPG", "JMPGU", "JMPL", "JMPLU", "JMPI", "JMPEI", "JMPNI", "JMPGI", "JMPGUI",
    "JMPLI", "JMPLUI", "CMP", "SHR", "SHL", "AND", "OR", "NOT", "XOR", "NEG", "RET", "HAL",
];

/// Flag bits deposited in reg_d by `CMP`.
pub const FLAG_EQ: u32 = 1;
pub const FLAG_GT: u32 = 2;
pub const FLAG_LT: u32 = 4;
pub const FLAG_GTU: u32 = 8;
pub const FLAG_LTU: u32 = 16;

/// `RET` in the current ISA does `popped + RET_BIAS` rather than returning to
/// a real saved address: the caller's instruction length is baked into the
/// callee. `JMP` (2 words) pushes its own address, and `JMPI` (1 word)
/// compensates by pushing `address - 1`. This is bug-compatible with
/// `MICA_inter_emu` on purpose, so binaries already assembled keep working.
/// See the design doc for why this should eventually become a plain
/// return-address push with `RET_BIAS = 0`.
pub const RET_BIAS: u32 = 2;

/// Number of words an instruction occupies, including its operand word.
///
/// NOTE: the indirect memory ops (`LDAI`/`LDBI`/`STAI`/`STBI`) are **one**
/// word here, because they carry no operand and the assembler emits them as a
/// single word. `MICA_inter_emu` currently advances the PC by 2 for all four,
/// which desynchronises any program that uses them. This table is the correct
/// one; the interpreter needs the matching fix.
pub fn encoded_len(opcode: u32) -> u32 {
    match opcode {
        // LD*/ST* direct forms and the direct jump family take an operand word.
        1 | 3 | 5 | 6 | 7 | 8 | 10 | 12 | 13 | 14 | 25..=31 => 2,
        _ => 1,
    }
}

/// How a decoded instruction ends (or does not end) a basic block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    /// Falls through to the next instruction.
    Linear,
    /// Unconditional call: pushes a return address, jumps to a literal target.
    Call { target: u32 },
    /// Conditional branch on a reg_d flag mask to a literal target.
    Branch {
        mask: u32,
        taken_when_set: bool,
        target: u32,
    },
    /// Unconditional indirect call through reg_c.
    CallIndirect,
    /// Conditional indirect branch through reg_c.
    BranchIndirect { mask: u32, taken_when_set: bool },
    /// Pop the shared stack and resume.
    Ret,
    /// Stop the machine.
    Halt,
    /// Opcode outside the table.
    Invalid,
}

#[derive(Clone, Copy, Debug)]
pub struct Insn {
    pub addr: u32,
    pub opcode: u32,
    /// The operand word, when `encoded_len == 2`.
    pub operand: u32,
    pub len: u32,
    pub flow: Flow,
}

impl Insn {
    pub fn mnemonic(&self) -> &'static str {
        MNEMONICS
            .get(self.opcode as usize)
            .copied()
            .unwrap_or("???")
    }
}

/// Decode the instruction at word address `addr`.
///
/// Reads at most two words and never traps: an address past the end of RAM or
/// an unknown opcode decodes to `Flow::Invalid`, which the engine turns into a
/// clean exit rather than a panic.
pub fn decode(ram: &[u32], addr: u32) -> Insn {
    let opcode = ram.get(addr as usize).copied().unwrap_or(u32::MAX);
    let len = encoded_len(opcode);
    let operand = if len == 2 {
        ram.get(addr as usize + 1).copied().unwrap_or(0)
    } else {
        0
    };

    let flow = match opcode {
        25 => Flow::Call { target: operand },
        26 => Flow::Branch {
            mask: FLAG_EQ,
            taken_when_set: true,
            target: operand,
        },
        27 => Flow::Branch {
            mask: FLAG_EQ,
            taken_when_set: false,
            target: operand,
        },
        28 => Flow::Branch {
            mask: FLAG_GT,
            taken_when_set: true,
            target: operand,
        },
        29 => Flow::Branch {
            mask: FLAG_GTU,
            taken_when_set: true,
            target: operand,
        },
        30 => Flow::Branch {
            mask: FLAG_LT,
            taken_when_set: true,
            target: operand,
        },
        31 => Flow::Branch {
            mask: FLAG_LTU,
            taken_when_set: true,
            target: operand,
        },
        32 => Flow::CallIndirect,
        33 => Flow::BranchIndirect {
            mask: FLAG_EQ,
            taken_when_set: true,
        },
        34 => Flow::BranchIndirect {
            mask: FLAG_EQ,
            taken_when_set: false,
        },
        35 => Flow::BranchIndirect {
            mask: FLAG_GT,
            taken_when_set: true,
        },
        36 => Flow::BranchIndirect {
            mask: FLAG_GTU,
            taken_when_set: true,
        },
        37 => Flow::BranchIndirect {
            mask: FLAG_LT,
            taken_when_set: true,
        },
        38 => Flow::BranchIndirect {
            mask: FLAG_LTU,
            taken_when_set: true,
        },
        47 => Flow::Ret,
        48 => Flow::Halt,
        o if (o as usize) < MNEMONICS.len() => Flow::Linear,
        _ => Flow::Invalid,
    };

    Insn {
        addr,
        opcode,
        operand,
        len,
        flow,
    }
}

/// True for opcodes that write to RAM. Used by block discovery to decide where
/// self-modifying-code guards are needed.
pub fn is_store(opcode: u32) -> bool {
    matches!(opcode, 8 | 9 | 10 | 11 | 12 | 13 | 14)
}

/// True for stores whose target address comes from reg_c at run time rather
/// than from an operand word (`STAI`, `STBI`).
pub fn is_indirect_store(opcode: u32) -> bool {
    matches!(opcode, 9 | 11)
}
