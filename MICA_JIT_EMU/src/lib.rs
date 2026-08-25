//! MICA JIT recompiling emulator.
//!
//! Layering, bottom-up:
//!
//! * `isa`     - opcode table, instruction lengths, decode. One source of truth.
//! * `cpu`     - guest state and the compiled-block ABI.
//! * `interp`  - tier-0 interpreter: runs cold code, and is the oracle the JIT
//!               is differentially tested against.
//! * `block`   - basic-block discovery.
//! * `codegen` - Cranelift translation of one block into one native function.
//! * `engine`  - the dispatcher and block cache that ties the tiers together.

pub mod block;
pub mod codegen;
pub mod config;
pub mod cpu;
pub mod engine;
pub mod interp;
pub mod isa;

pub use cpu::{Exit, Machine};
pub use engine::{Engine, EngineConfig};
