# R.4b — proving `RefBackend = ℤ/ℚ` in Lean: feasibility (GO) + the proof plan

*Status: **R.4b.0 spike — GO.** The reference bignum (`crates/lattice/src/refbackend.rs`) lifts
cleanly through charon+aeneas at the pinned toolchain; the proof machinery is the established
`loop.spec_decr_nat` + `step` idiom over a limb→ℕ denotation. The op proofs themselves (R.4b.1+) are
a large, incremental verification effort — each op a loop-invariant proof — tracked below.*

## The feasibility GO (spike)

`charon cargo --preset=aeneas --start-from crate::refbackend` + `aeneas` both exit 0 — no crash, no
modeling wall. Concretely:
- **Types lift as expected:** `refbackend.RefNat = { limbs : alloc.vec.Vec Std.U64 }`,
  `RefInt = { neg : Bool, mag : RefNat }`, `RefRat = { num : RefInt, den : RefNat }`. The denotation
  lands directly on `limbs`.
- **All ops lift** (~1450 lines of `Funs`): every loop emits the clean `<op>_loop` / `<op>_loop.body`
  shape (`add_loop`, `cmp_loop`, `sub_loop`, `shl1_loop`, `mul_loop0` + nested `mul_loop0_loop0/1`,
  `divrem_loop`, `gcd_loop`, `normalize_loop`). The `Backend` structure is dup-free (R.3a paid off).
- **`u128` carry is NATIVE** — Aeneas models the `u128` add/mul/shift/cast scalar ops itself; they are
  NOT externalised. This was the main risk; it's clear. Reasoning goes through Aeneas's scalar lemmas.
- **Small, faithful external surface** (the only hand-written TCB): `Vec.pop`, `Vec.is_empty`,
  `i128.wrapping_neg`, `i128.unsigned_abs` (already modelled), `usize.div_ceil`,
  `Ordering::eq`, and a `MaybeUninit` phantom type — all one-line faithful `def`s.

## Proof plan (denotation + per-op loop invariant)

Denotation (little-endian base 2⁶⁴), over limb values as ℕ: `den [] = 0`,
`den (x::xs) = x + 2⁶⁴·den xs`; on the lift, `den (limbs.val.map U64.val)`. Core lemmas:
`den (l ++ [0]) = den l` ⇒ `den (normalize l) = den l`; `den l < 2^(64·len)`; normalized ⇒ len
monotone in value ⇒ `den` injective on normalized lists (RefNat value ≅ ℕ).

| op | target | invariant / method |
|---|---|---|
| `is_zero` | `= true ↔ den = 0` | direct (normalized `Vec.is_empty`) |
| `cmp` | `= compare (den a) (den b)` | len-then-MSB-lex = ℕ order for normalized |
| `add` | `den = den a + den b` | carry loop; u128 split `out_i + 2⁶⁴·carry₊ = a_i+b_i+carry` |
| `sub` (a≥b) | `den = den a − den b` | dual borrow loop |
| `mul` | `den = den a · den b` | nested loop; row-accumulate; carry ≤ 2¹²⁸−1 |
| `divrem` | `den self = den q·den d + den r`, `r<d` | bit-serial; `den(shl1 x)=2·den x` |
| `gcd` | `= Nat.gcd (den a) (den b)` | Euclid; measure `den y`; `Nat.gcd_rec` + divrem's `%` |
| `RefInt` | `intDen = ℤ` ops | sign-magnitude case split; divrem = `Int.tdiv/tmod` |
| `RefRat` | `ratDen = ℚ` ops | `intDen num / den`; `reduce` = lowest terms via gcd |

## Phasing (each phase = a committed, axiom-audited proof; pause per phase)
`R.4b.1` model wiring + denotation + `is_zero`/`cmp` · `.2` `add`/`sub` · `.3` `mul` · `.4` `divrem`
· `.5` `gcd` · `.6` `RefInt`/`RefRat` → ℤ/ℚ + the `Backend`-instance corollary.

R.4b.1 also wires the committed extraction (a `refbackend` start-from + the model + the ~7 external
models, drift-checked like the other lifts). The `u128`-scalar reasoning in `add` prices the hardest
recurring sub-lemma; `mul` (nested) and `divrem` (bit-serial) are the effort peaks.
