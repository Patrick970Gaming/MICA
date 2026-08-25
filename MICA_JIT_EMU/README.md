# MICA_JIT_EMU

A recompiling emulator for MICA. It translates guest basic blocks into native
machine code with [Cranelift](https://cranelift.dev/) and runs them directly,
falling back to an interpreter for code that has not run often enough to be
worth compiling.

It is deliberately separate from `MICA_inter_emu`: that emulator stays the
simple, readable reference implementation, and this one is checked against it.

## Build and run

```sh
cd MICA_JIT_EMU
cargo build --release

# assemble something first (from the repo root)
python3 Assembler/assembler.py

./target/release/mica_jit_emu ../Assembler/output32.bin --code-limit 26
```

Flags:

| Flag | Meaning |
|---|---|
| `--config PATH` | config file, default `./config.json` (same schema as the interpreter's) |
| `--code-limit N` | word address where code ends and the constant pool starts — see below |
| `--interp` | tier 0 only, no compilation |
| `--trace` | print a line per compiled block |
| `--diff` | run both tiers and report the first divergence |

`--diff` is the important one during development. It runs the image twice from
an identical start state — once purely interpreted, once with every block
compiled — and compares registers, stack and every word of RAM:

```
$ ./target/release/mica_jit_emu fib32.bin --diff --code-limit 26
interp: Halt @25 after 141 insns
jit:    Halt @25  (blocks_compiled: 2, native_entries: 11, cache_flushes: 0)
identical: registers, stack and all 65536 RAM words match
```

## Why `--code-limit` matters

MICA's image format has no header, so nothing in the binary says where code
stops and the constant pool begins. Without that boundary the engine must
assume the whole image is code and treat every store into it as potential
self-modifying code. On `fib.masm` that turns 2 compiled blocks and 0 cache
flushes into 51 compiled blocks and 40 flushes — every write to a variable
throws away the entire translation cache.

The assembler already computes this number (`total_function_len`). Emitting a
small image header is the single highest-value change to the toolchain for the
JIT's sake; until then, pass it on the command line.

## Layout

```
src/isa.rs      opcode table, instruction lengths, decode — one source of truth
src/cpu.rs      guest state, and the ABI compiled blocks are called through
src/interp.rs   tier-0 interpreter: runs cold code, and is the differential oracle
src/block.rs    basic-block discovery
src/codegen.rs  Cranelift translation of one block into one native function
src/engine.rs   dispatcher, block cache, hotness policy, SMC invalidation
tests/          differential tests, including a randomised fuzz loop
```

Both tiers decode through `isa.rs`, so they cannot disagree about what an
instruction *is* — only about what it *does*, which is exactly what the
differential tests check.

## Semantics this emulator pins down

Three things are undefined or accidental in `MICA_inter_emu`. A recompiler
cannot leave them undefined, because Cranelift's choice and Rust's choice
differ, so this crate fixes them and the interpreter should be updated to
match:

| Case | `MICA_inter_emu` today | Here |
|---|---|---|
| Integer overflow on `ADD`/`SUB`/`MUL` | panics in a debug build, wraps in release | always wraps |
| Division by zero | panics | yields 0 |
| Address outside RAM | panics on index | masked into range (RAM size must be a power of two) |

`RET`'s `popped + 2` convention and `JMPI`'s compensating `-1` are reproduced
exactly as they are, so binaries assembled today keep working unchanged.
