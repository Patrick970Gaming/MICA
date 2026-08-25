# A JIT recompiler for MICA — proposed method

Status: scaffold implemented and passing differential tests against the
interpreter on all three example programs. This document explains the method
and what remains.

---

## 1. The recommendation in one paragraph

Compile **one guest basic block at a time, lazily, on its second-or-later
execution**, into a native function that Cranelift generates; keep an
interpreter as tier 0 for cold code; and have every compiled block return the
next guest PC to a dispatcher that owns a block cache. Guest registers become
SSA values for the lifetime of a block and live in host registers, which is
where nearly all of the speedup comes from on an accumulator machine like MICA.
Correctness is held down by a differential harness that runs both tiers from
the same state and compares every register, the stack, and every word of RAM.

## 2. Why not whole-program recompilation

`Python/Recompiler/recompiler.py` sketches two whole-program approaches:
emit a function per jump target and drive them from a main loop, or split the
program at every jump into function-sized chunks ahead of time. Both are
reasonable first instincts and both break on MICA specifically:

- **`JMPI` and its six conditional siblings jump to whatever is in `reg_c`.**
  A static pass cannot enumerate the targets. Any ahead-of-time chunking has to
  fall back to a runtime dispatch table anyway — at which point you have built
  the dispatcher, just with worse coverage.
- **Code and data share one address space with no boundary marker.** `STA` can
  write to a code address. Whole-program translation is only valid until the
  first such store; a lazy translator simply re-translates.
- **The constant pool sits immediately after the code.** A static scan has no
  way to know where instructions stop, so it will "decode" the pool as
  instructions and generate garbage for code that never runs.

Lazy block compilation sidesteps all three: you only ever translate bytes you
are about to execute, so you never guess about reachability, and every
translation is invalidated cheaply if the underlying memory changes.

The one place ahead-of-time compilation *does* fit is as a later addition: once
the block translator exists, pointing it at an object-file backend instead of
the JIT backend gives you an AOT compiler for free, for programs that don't
self-modify. Cranelift supports both from the same IR.

## 3. Pipeline

```
guest PC
   │
   ▼
dispatcher ── in block cache? ──yes──▶ call native block ──┐
   │ no                                                     │
   ▼                                                        │
counter++ ── below threshold? ──yes──▶ interpret one block ─┤
   │ no                                                     │
   ▼                                                        │
discover block ─▶ translate to Cranelift IR ─▶ compile ─▶ cache ─▶ call ─┘
                                                                    │
                                        returns (exit_reason, next_pc)
```

**Block discovery** walks forward from an address until it hits a jump, `RET`,
`HAL`, an undecodable opcode — or a store that might land in the code region,
because from that instruction on, the bytes being translated may not be the
bytes that will run.

**Translation** is a single linear pass. There is no control flow inside a
block except the two-way split at a conditional terminator, so no phi nodes and
no dominance analysis are needed; block-local values are just Rust variables
holding `cranelift::Value`s.

**Compilation** hands the IR to Cranelift, which does register allocation,
instruction selection and peephole optimisation for x86-64, aarch64 or riscv64
without any target-specific code on your side.

## 4. Where the speed actually comes from

Not from Cranelift's optimiser. From the register mapping.

MICA is an accumulator machine: `LDA x / LDB y / ADD / STC z` is the shape of
almost everything. An interpreter turns each of those into a memory read of the
register field, an operation, and a memory write — plus an opcode dispatch. A
block translator loads `reg_a..reg_e` and `sp` **once** at block entry, keeps
them in host registers across the whole block, and writes them back **once** per
exit edge. Constant folding then falls out for free: `LDA` of a constant-pool
address is a load from a known address, so Cranelift can often propagate the
value into the arithmetic that follows.

The second-order win is that opcode dispatch disappears entirely — there is no
`match` at run time, because the shape of the block is baked into the code.

## 5. The block ABI

```rust
extern "C" fn(cpu: *mut CpuState, ram: *mut u32) -> u64
```

The return value is `(exit_reason << 32) | next_pc`, so a block returns through
a single integer register with no memory traffic. Exit reasons are `Continue`,
`Halt`, `Invalid`, `SelfModified`.

Two pointers rather than one because RAM is a growable `Vec` and `CpuState` is
not; keeping them separate avoids a level of indirection on every guest memory
access.

**Memory access** is `ram + (addr & mask) * 4`, with the mask a compile-time
constant. Forcing `ram_size` to a power of two buys bounds safety for one `AND`
and no branch — which matters, because a guest store is otherwise the cheapest
instruction in the set.

## 6. Control flow

| Terminator | How it is translated |
|---|---|
| `JMP` (call) | push return address, return `Continue(target)` |
| `JMPE`…`JMPLU` | test the flag bit, two-way `brif`, each arm returns its constant PC |
| `JMPI` (indirect call) | push biased return address, return `Continue(reg_c)` |
| `JMPEI`…`JMPLUI` | two-way `brif`, taken arm returns `reg_c` |
| `RET` | pop, return `Continue(popped + 2)` |
| `HAL` | return `Halt` |

Everything returns to the dispatcher, which costs an indirect call plus a hash
lookup per block. That is the correct starting point and the first thing to
optimise once correctness is established:

- **Direct chaining.** When a terminator's targets are compile-time constants
  and both are already compiled, emit a direct tail call instead of a return.
  `Block::static_successors()` already computes the candidate set.
- **Indirect branch cache.** `JMPI` and `RET` targets are dynamic but highly
  repetitive. A small direct-mapped `(guest_pc → native_ptr)` cache checked
  inline, falling back to the dispatcher on a miss, removes most of the
  remaining dispatch cost.
- **Superblocks.** Once loops are hot, extending a block through a
  predictably-taken branch (with a guard that exits on the unexpected
  direction) turns the fib loop body into one straight-line function.

Do these in that order, measuring each. None of them are worth starting before
the differential harness is green.

## 7. Self-modifying code

The guest can write to any address, including its own code. The scheme here:

- A store whose address is a compile-time constant **at or above** `code_limit`
  cannot touch code. It costs nothing extra.
- Any other store gets an inline `addr < code_limit` test whose result becomes
  the block's exit code — branchlessly, by OR-ing `SelfModified` into the packed
  return value. No branch is added to the hot path.
- Block discovery ends a block at such a store, so a block can never execute
  past code it may have just rewritten.
- On a `SelfModified` exit the dispatcher flushes the cache and re-discovers.

Flushing everything is coarse. It is also correct, and MICA programs are small.
The refinement is a dirty bitmap at, say, 64-word page granularity, invalidating
only blocks that overlap a dirtied page — worth doing when a real program
actually self-modifies in a loop, not before.

**This is where the missing image header hurts.** With no code/data boundary in
the binary, `code_limit` has to be the whole image, so every write to a variable
looks like self-modification. On `fib.masm` that is the difference between
2 compiled blocks / 0 flushes and 51 compiled blocks / 40 flushes. See §9.

## 8. Semantics that have to be pinned down

A recompiler cannot run on undefined behaviour, because the host and the
interpreter will make different choices and the differential test will fail —
correctly. Three cases in `MICA_inter_emu` are currently accidental rather than
specified. The scaffold picks a definition for each; the interpreter should be
changed to match, so that the ISA has one meaning.

| Case | Today | Proposed |
|---|---|---|
| `ADD`/`SUB`/`MUL` overflow | panics in a debug build, wraps in release | **wraps**, always |
| `DIV` by zero | panics | **yields 0** (branchless: divide by a substituted 1, then select) |
| Address ≥ `ram_size` | panics on the index | **masked** into range; `ram_size` must be a power of two |

Wrapping is the right call because Cranelift's `iadd` wraps and reproducing a
panic faithfully would mean emitting an overflow check on every arithmetic
instruction. Defining division by zero costs one `select` instead of a branch
and a trap path. Masking makes every guest memory access one `AND` instead of a
compare-and-branch.

`RET`'s `popped + 2` and `JMPI`'s compensating `-1` are reproduced exactly as
they are, so existing binaries keep working.

## 9. Issues found in the review that affect the JIT

1. **`LDAI`/`LDBI`/`STAI`/`STBI` have two different lengths.** The assembler
   emits them as one word (they carry no operand); `MICA_inter_emu` advances
   the PC by **2** for all four. Any program that uses them desynchronises after
   the first one. The scaffold's `isa::encoded_len` uses 1 word, which matches
   the assembler; the interpreter's four `current_address += 2` need to become
   `+= 1`. **This is a real bug independent of the JIT** — it just happens that
   a recompiler cannot start without an authoritative length table, which is
   what surfaced it.

2. **The image has no header.** Nothing distinguishes code from the constant
   pool, so the JIT must assume the whole image is code. The assembler already
   knows the boundary (`total_function_len`). A small header — magic, version,
   bit width, entry point, code length — is the highest-value toolchain change
   for the recompiler, and it costs about ten lines in `assembler.py`. Until
   then `--code-limit N` is a workaround.

3. **`RET` bakes the caller's instruction length into the callee.** It works,
   and `JMPI` compensates correctly, but it means a return address on the stack
   is not a real address, which blocks the obvious return-address-stack
   optimisation later and makes `PSH`/`PLL` interleaving harder to reason about.
   Worth changing to "push the actual return address, `RET` uses it directly"
   at the next point you are willing to rebuild binaries.

4. **Return addresses and `PSH`/`PLL` data share one 512-word array with no
   bounds check.** An unbalanced push corrupts the next `RET`. For the JIT this
   means `RET` is a fully dynamic branch that can never be statically chained.
   A separate call stack would make returns predictable and make stack overflow
   detectable.

5. **No I/O instructions.** Programs can only communicate through the final
   memory dump. Before the JIT is worth benchmarking, MICA needs some way for a
   program to produce output — a memory-mapped region is the cheapest option and
   fits the JIT model well (a store to a guarded address range exits to the
   dispatcher, exactly like the SMC guard already does).

6. **`reg_e` (segment) is loaded and stored but never interpreted.** Not urgent,
   but decide what it means before code depends on it.

7. Cosmetic, from the interpreter: the unconditional `println!` in `RET`'s match
   arm and the raw byte-vector dump in `load_image_into_ram` are not gated by
   `verbose`.

## 10. Verification

This is the part that makes the difference between a JIT you trust and one you
don't.

- **Differential testing at block granularity.** Run both tiers from an
  identical start state; after every compiled block, step the interpreter to the
  same guest PC and compare all registers, the stack and all of RAM. A failure
  names the block that broke, not the end of the program.
- **A randomised fuzz loop.** Thousands of random straight-line programs over
  the arithmetic, bitwise and addressing opcodes, seeded with values chosen to
  hit zero, signed boundaries and `u32::MAX`. This is what catches flag-
  computation and wrap-around mistakes.
- **An exhaustive `CMP` matrix** over the eight interesting values, checked
  against the interpreter's flag computation. `CMP` is emitted branchlessly as
  five predicates OR-ed together, which is exactly the sort of code that is
  subtly wrong at signed/unsigned boundaries.
- **Real programs.** `fib.masm`, `hello.masm` and `assembly.masm` (the
  `LDC`/`JMPI`/`RET` one) assembled by the real assembler and run through
  `--diff`.

All of the above are implemented and passing. The register output was also
cross-checked against a real `MICA_inter_emu` run of `hello.masm`:
`a=420 b=420 c=840 d=0 e=0 sp=0` from both.

## 11. Milestones

| | | Status |
|---|---|---|
| M0 | Shared decoder, guest state, tier-0 interpreter | **done** |
| M1 | Block discovery + Cranelift translation of the full opcode set | **done** |
| M2 | Dispatcher, block cache, hotness policy | **done** |
| M3 | SMC detection and cache flush | **done** (coarse) |
| M4 | Differential harness + fuzz + real programs | **done** |
| M5 | Image header in the assembler; fix the `*I` instruction lengths | next |
| M6 | Direct block chaining + indirect-branch cache | then |
| M7 | Superblock formation across hot branches | then |
| M8 | Page-granular invalidation; AOT object-file backend | later |

M5 is small, mechanical, and unblocks real performance work — do it first.

## 12. What to measure

Once M5 lands, the number that matters is guest instructions per second on a
loop-heavy program, JIT versus `--interp`, at a `hot_threshold` swept from 0
upward. Expect the JIT to lose on programs that halt in a few hundred
instructions (compile cost dominates) and to win by an order of magnitude or
more on anything that loops — which is the correct shape, and is why the
hotness threshold exists at all.

Watch `blocks_compiled` and `cache_flushes` in the stats line. If flushes are
nonzero on a program that doesn't intentionally self-modify, `code_limit` is
wrong, and the JIT is doing throwaway work.
