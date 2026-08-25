//! Basic-block discovery.
//!
//! A block starts at a guest address and runs forward until something makes
//! the next PC non-obvious. In MICA that is: any jump (direct or indirect),
//! `RET`, `HAL`, an undecodable opcode — and, less obviously, any store that
//! might land in the code region, because from that instruction onward the
//! bytes we are translating may no longer be the bytes that will run.

use crate::isa::{self, Flow, Insn};

pub struct Block {
    pub start: u32,
    /// Straight-line body, not including the terminator.
    pub body: Vec<Insn>,
    /// The instruction that ends the block. `None` means the block was cut for
    /// length or by a self-modifying store, and simply falls through.
    pub terminator: Option<Insn>,
    /// Address to resume at when the block falls off the end.
    pub fallthrough: u32,
    /// Total guest instructions, terminator included.
    pub len: usize,
}

/// Blocks are capped so that one pathological straight-line run cannot make a
/// single compilation take unbounded time. Anything longer is split and the
/// pieces chain through the dispatcher.
pub const MAX_BLOCK_INSNS: usize = 256;

pub fn discover(ram: &[u32], start: u32, code_limit: u32) -> Block {
    let mut body = Vec::new();
    let mut pc = start;

    for _ in 0..MAX_BLOCK_INSNS {
        let insn = isa::decode(ram, pc);
        let next = pc.wrapping_add(insn.len);

        if insn.flow != Flow::Linear {
            return Block {
                start,
                len: body.len() + 1,
                body,
                terminator: Some(insn),
                fallthrough: next,
            };
        }

        // A store into the code region invalidates translations, possibly this
        // one. Rather than reason about which, end the block here: the
        // dispatcher will flush and re-discover from `next` against whatever
        // the memory now says.
        let cuts_block = isa::is_store(insn.opcode)
            && (isa::is_indirect_store(insn.opcode) || insn.operand < code_limit);

        body.push(insn);
        pc = next;

        if cuts_block {
            break;
        }
    }

    let len = body.len();
    Block {
        start,
        body,
        terminator: None,
        fallthrough: pc,
        len,
    }
}

impl Block {
    /// The successor addresses that are known at translation time. Used later
    /// for direct block chaining; indirect targets and `RET` never appear here
    /// and always go back through the dispatcher.
    pub fn static_successors(&self) -> Vec<u32> {
        match self.terminator.map(|t| t.flow) {
            Some(Flow::Call { target }) => vec![target],
            Some(Flow::Branch { target, .. }) => vec![target, self.fallthrough],
            Some(Flow::Linear) | None => vec![self.fallthrough],
            _ => vec![],
        }
    }
}
