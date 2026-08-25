//! Differential tests: the JIT must be indistinguishable from the tier-0
//! interpreter.
//!
//! Every test builds an image, runs it twice from an identical start state -
//! once purely interpreted, once with `hot_threshold = 0` so every block is
//! compiled - and compares registers, stack and the whole of RAM. `lockstep`
//! goes further and compares after every single block, so a failure points at
//! the block that broke rather than at the end of the program.

use mica_jit_emu::cpu::{Exit, Machine};
use mica_jit_emu::{engine, interp, isa};

const RAM: usize = 4096;

// Opcodes, by name, so the tests read like assembly.
const NOP: u32 = 0;
const LDA: u32 = 1;
const LDAI: u32 = 2;
const LDB: u32 = 3;
const LDC: u32 = 5;
const STA: u32 = 8;
const STAI: u32 = 9;
const STC: u32 = 12;
const PSH: u32 = 15;
const PLL: u32 = 16;
const ADD: u32 = 17;
const SUB: u32 = 18;
const MUL: u32 = 19;
const DIV: u32 = 20;
const JMP: u32 = 25;
const JMPN: u32 = 27;
const JMPI: u32 = 32;
const CMP: u32 = 39;
const SHR: u32 = 40;
const AND: u32 = 42;
const XOR: u32 = 45;
const NEG: u32 = 46;
const RET: u32 = 47;
const HAL: u32 = 48;

fn machine(words: &[u32]) -> Machine {
    let mut m = Machine::new(RAM);
    let mut bytes = Vec::with_capacity(words.len() * 4);
    for w in words {
        bytes.extend_from_slice(&w.to_be_bytes());
    }
    m.load_image(&bytes).expect("image fits");
    m
}

fn clone_of(src: &Machine) -> Machine {
    let mut m = Machine::new(src.ram.len());
    m.ram.copy_from_slice(&src.ram);
    m.code_limit = src.code_limit;
    m.cpu = src.cpu.clone();
    m
}

fn compare(interp_m: &Machine, jit_m: &Machine, where_: &str) {
    assert_eq!(
        interp_m.cpu, jit_m.cpu,
        "{where_}: cpu diverged\n  interp {:?}\n  jit    {:?}",
        interp_m.cpu, jit_m.cpu
    );
    for (addr, (x, y)) in interp_m.ram.iter().zip(jit_m.ram.iter()).enumerate() {
        assert_eq!(
            x, y,
            "{where_}: ram[{addr}] diverged: interp {x} vs jit {y}"
        );
    }
}

/// Run both tiers to completion and compare the end state.
fn assert_agree(words: &[u32]) -> Machine {
    let template = machine(words);
    let mut a = clone_of(&template);
    let mut b = clone_of(&template);

    let (exit_i, pc_i, _) = interp::run(&mut a, 0, 1_000_000);

    let mut eng = engine::Engine::new(engine::EngineConfig {
        hot_threshold: 0,
        max_steps: 1_000_000,
        trace: false,
    })
    .expect("jit init");
    let (exit_j, pc_j) = eng.run(&mut b, 0).expect("jit run");

    assert_eq!((exit_i, pc_i), (exit_j, pc_j), "exit diverged");
    compare(&a, &b, "final");
    b
}

/// Compare after every block, so the first divergence names its block.
fn assert_agree_lockstep(words: &[u32]) {
    let template = machine(words);
    let mut a = clone_of(&template);
    let mut b = clone_of(&template);

    let mut eng = engine::Engine::new(engine::EngineConfig {
        hot_threshold: 0,
        max_steps: 1_000_000,
        trace: false,
    })
    .expect("jit init");

    let mut pc = 0u32;
    for block_no in 0..10_000 {
        let (exit_j, next_j) = eng.step_block(&mut b, pc).expect("jit block");

        // Walk the interpreter forward until it reaches the same guest PC.
        let mut ipc = pc;
        let mut exit_i = Exit::Continue;
        for _ in 0..1024 {
            let (e, n) = interp::step(&mut a, ipc);
            ipc = n;
            exit_i = e;
            if ipc == next_j || !matches!(e, Exit::Continue | Exit::SelfModified) {
                break;
            }
        }

        assert_eq!(
            ipc, next_j,
            "block {block_no} from pc {pc}: next-pc diverged"
        );
        compare(&a, &b, &format!("after block {block_no} from pc {pc}"));

        if exit_j == Exit::SelfModified {
            eng.flush();
            b.cpu.smc_dirty = 0;
            a.cpu.smc_dirty = 0;
        } else if exit_j != Exit::Continue {
            assert_eq!(exit_i, exit_j, "block {block_no}: exit reason diverged");
            return;
        }
        pc = next_j;
    }
    panic!("program did not terminate within 10000 blocks");
}

#[test]
fn straight_line_arithmetic() {
    // data at 12: x=7, y=5, out=0
    let p = vec![
        LDA, 12, // reg_a = 7
        LDB, 13,  // reg_b = 5
        ADD, //       reg_c = 12
        STC, 14,  //   out = 12
        SUB, //       reg_c = 2
        MUL, //       reg_c = 35
        DIV, //       reg_c = 1
        HAL, 0, // (pad so data starts at 12)
        7, 5, 0,
    ];
    let m = assert_agree(&p);
    assert_eq!(m.ram[14], 12);
    assert_eq!(m.cpu.reg_c, 7 / 5);
}

#[test]
fn division_by_zero_is_defined_not_a_trap() {
    let p = vec![LDA, 8, LDB, 9, DIV, HAL, 0, 0, 42, 0];
    let m = assert_agree(&p);
    assert_eq!(m.cpu.reg_c, 0, "DIV by zero is defined as producing 0");
}

#[test]
fn bitwise_and_unary_ops() {
    let p = vec![
        LDA, 14, LDB, 15, AND, LDA, 14, LDB, 15, XOR, SHR, NEG, HAL, 0, 0xF0F0, 0x0FF0,
    ];
    assert_agree(&p);
}

#[test]
fn cmp_flag_matrix_matches_interpreter() {
    // Signed/unsigned boundaries are where a hand-written flag computation
    // usually goes wrong, so cover them explicitly rather than randomly.
    let interesting: [u32; 8] = [
        0,
        1,
        2,
        0x7FFF_FFFF,
        0x8000_0000,
        0x8000_0001,
        0xFFFF_FFFE,
        u32::MAX,
    ];
    for x in interesting {
        for y in interesting {
            let p = vec![LDA, 8, LDB, 9, CMP, HAL, 0, 0, x, y];
            let m = assert_agree(&p);
            assert_eq!(
                m.cpu.reg_d,
                interp::compare(x, y),
                "CMP {x:#x} vs {y:#x} produced the wrong flags"
            );
        }
    }
}

#[test]
fn fib_loop_matches() {
    // A transcription of Assembler/fib.masm: a real loop, so the block at the
    // top is entered many times and the branch terminator is exercised both
    // ways.
    const A: u32 = 26;
    const B: u32 = 27;
    const F: u32 = 28;
    const COUNT: u32 = 29;
    const LIMIT: u32 = 30;
    const ONE: u32 = 31;
    const STB: u32 = 10;
    let p = vec![
        LDA, A, LDB, B, ADD, STC, F, //  0: f = a + b
        STB, A, //                       7: a = b
        STC, B, //                       9: b = f
        LDA, COUNT, LDB, ONE, ADD, STC, COUNT, // 11: count += 1
        LDA, COUNT, LDB, LIMIT, CMP, //  18: compare count, limit
        JMPN, 0,   //                     23: loop while not equal
        HAL, //                          25
        0, 1, 0, 0, 10, 1, //            26: a b f count limit one
    ];
    assert_agree_lockstep(&p);
    let m = assert_agree(&p);
    assert_eq!(m.ram[COUNT as usize], 10, "loop ran to the limit");
}

#[test]
fn call_and_return() {
    // main: LDA/LDB/ADD/STC, JMP sub, HAL   sub: LDA/LDB/ADD/STC, RET
    const R1: u32 = 22;
    const R2: u32 = 23;
    const X: u32 = 24;
    const Y: u32 = 25;
    let p = vec![
        LDA, X, LDB, Y, ADD, STC, R1, // 0..6
        JMP, 10,  // 7..8  -> sub at 10
        HAL, // 9
        LDA, Y, LDB, Y, ADD, STC, R2, RET, // 10..17
        NOP, NOP, NOP, NOP, // padding to 22
        0, 0, 3, 4,
    ];
    let m = assert_agree(&p);
    assert_eq!(m.ram[R1 as usize], 7);
    assert_eq!(m.ram[R2 as usize], 8);
    assert_eq!(m.cpu.sp, 0, "RET popped the frame JMP pushed");
}

#[test]
fn indirect_call_through_reg_c() {
    // LDC @sub (via the constant pool), JMPI, HAL; sub: RET
    const POOL: u32 = 8; // holds the code address of `sub`
    const OUT: u32 = 9;
    let p = vec![
        LDC, POOL, // reg_c = 6 (address of sub)
        JMPI, //     1-word indirect call
        STC, OUT, // proves RET landed here and not mid-instruction
        HAL, //
        RET, // 6: sub
        NOP, // 7
        6, 0,
    ];
    let m = assert_agree(&p);
    assert_eq!(
        m.ram[OUT as usize], 6,
        "RET resumed at the instruction after JMPI"
    );
    assert_eq!(m.cpu.sp, 0);
}

#[test]
fn indirect_load_and_store() {
    const PTR: u32 = 10;
    const SRC: u32 = 11;
    const DST: u32 = 12;
    let p = vec![
        LDC, PTR,  // reg_c = 11
        LDAI, //     reg_a = ram[11] = 99
        LDC, DST, // reg_c is loaded from ram[12]... see below
        HAL, NOP, NOP, NOP, NOP, //
        11, 99, 12,
    ];
    assert_agree(&p);

    // And the store side: write reg_a through reg_c.
    let p2 = vec![LDA, 9, LDC, 10, STAI, HAL, NOP, NOP, NOP, 55, 8];
    let m = assert_agree(&p2);
    assert_eq!(m.ram[8], 55, "STAI wrote through reg_c");
}

#[test]
fn push_and_pull() {
    let p = vec![LDA, 10, PSH, LDA, 11, PSH, PLL, STA, 12, HAL, 4, 9, 0];
    let m = assert_agree(&p);
    assert_eq!(m.ram[12], 9);
    assert_eq!(m.cpu.sp, 1);
}

#[test]
fn self_modifying_store_forces_a_flush() {
    // Overwrite the NOP at address 4 with HAL, then run into it. If the JIT
    // did not invalidate, it would execute the stale NOP and fall through.
    let p = vec![
        LDA, 8, // reg_a = 48 (HAL)
        STA, 4,   // patch address 4
        NOP, // 4: becomes HAL at run time
        LDA, 9, STA, // deliberately never reached if the patch took effect
        48, 7,
    ];
    let mut eng_words = p.clone();
    eng_words.resize(12, 0);

    let template = machine(&eng_words);
    let mut b = clone_of(&template);
    let mut eng = engine::Engine::new(engine::EngineConfig {
        hot_threshold: 0,
        max_steps: 10_000,
        trace: false,
    })
    .unwrap();
    let (exit, pc) = eng.run(&mut b, 0).unwrap();
    assert_eq!(exit, Exit::Halt, "the patched HAL executed");
    assert_eq!(pc, 4);
    assert!(
        eng.stats.cache_flushes > 0,
        "the code store flushed the cache"
    );

    assert_agree_lockstep(&eng_words);
}

#[test]
fn block_discovery_stops_at_terminators() {
    let words = vec![LDA, 6, ADD, JMP, 0, HAL, 0];
    let m = machine(&words);
    let blk = mica_jit_emu::block::discover(&m.ram, 0, m.code_limit);
    assert_eq!(blk.body.len(), 2, "LDA and ADD are the straight-line body");
    assert_eq!(blk.terminator.map(|t| t.opcode), Some(JMP));
    assert_eq!(blk.fallthrough, 5);
}

#[test]
fn encoded_lengths_agree_with_the_assembler() {
    // The assembler emits one word for any mnemonic written without an
    // operand, which is every instruction outside these two families.
    for (op, name) in isa::MNEMONICS.iter().enumerate() {
        let expected = if matches!(
            *name,
            "LDA"
                | "LDB"
                | "LDC"
                | "LDD"
                | "LDE"
                | "STA"
                | "STB"
                | "STC"
                | "STD"
                | "STE"
                | "JMP"
                | "JMPE"
                | "JMPN"
                | "JMPG"
                | "JMPGU"
                | "JMPL"
                | "JMPLU"
        ) {
            2
        } else {
            1
        };
        assert_eq!(
            isa::encoded_len(op as u32),
            expected,
            "{name} (opcode {op}) has the wrong encoded length"
        );
    }
}

/// A tiny deterministic PRNG - no dev-dependency needed for a fuzz loop.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
}

#[test]
fn fuzz_straight_line_programs() {
    // Random straight-line bodies terminated by HAL. This is where the two
    // tiers' arithmetic and addressing are most likely to drift apart, and it
    // costs nothing to check a few thousand of them.
    const OPS: &[u32] = &[
        NOP, ADD, SUB, MUL, DIV, AND, XOR, SHR, NEG, CMP, PSH, PLL, LDAI, STAI,
    ];
    let mut rng = Rng(0x1234_5678_9abc_def0);

    for case in 0..2000 {
        let body_len = 4 + (rng.next() % 20) as usize;
        let mut prog: Vec<u32> = Vec::new();
        // Seed the registers from a data area placed after the code.
        let data_base = 64u32;
        prog.extend_from_slice(&[LDA, data_base, LDB, data_base + 1, LDC, data_base + 2]);

        for _ in 0..body_len {
            let op = OPS[(rng.next() as usize) % OPS.len()];
            prog.push(op);
        }
        prog.push(HAL);
        prog.resize(data_base as usize, NOP);
        // Values chosen to hit zero, signed boundaries and ordinary numbers.
        let pool = [0u32, 1, 0x8000_0000, u32::MAX, 12345, 70];
        prog.push(pool[(rng.next() as usize) % pool.len()]);
        prog.push(pool[(rng.next() as usize) % pool.len()]);
        prog.push(pool[(rng.next() as usize) % pool.len()]);
        prog.resize(data_base as usize + 16, 0);

        let template = machine(&prog);
        let mut a = clone_of(&template);
        let mut b = clone_of(&template);

        let (exit_i, pc_i, _) = interp::run(&mut a, 0, 10_000);
        let mut eng = engine::Engine::new(engine::EngineConfig {
            hot_threshold: 0,
            max_steps: 10_000,
            trace: false,
        })
        .unwrap();
        let (exit_j, pc_j) = eng.run(&mut b, 0).unwrap();

        assert_eq!((exit_i, pc_i), (exit_j, pc_j), "case {case}: exit diverged");
        compare(&a, &b, &format!("fuzz case {case}"));
    }
}
