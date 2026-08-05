# R.4b — proving `RefBackend = ℤ/ℚ` in Lean: feasibility (GO) + the proof plan

*Status: **R.4b.1 landed.** The reference bignum (`crates/lattice/src/refbackend.rs`) is lifted and
built into the committed `Lattice` model; `CertifyCheck/RefBackend.lean` proves the denotation lemmas
and the first two ops — `is_zero` (`= true ↔ den = 0`) and `cmp` (`= compare (den ·) (den ·)`) —
axiom-clean over a limb→ℕ denotation. The remaining ops (R.4b.2+) are a large, incremental
verification effort — each op a loop-invariant proof — tracked below.*

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
`R.4b.1` model wiring + denotation + `is_zero`/`cmp` **— done** · `.2` `add`/`sub` **— done**
(`normalize` den-preservation, `add` = the u128 carry loop, `sub` = the i128-borrow dual over ℤ) ·
`.3` `mul` **— done** (nested schoolbook multiply; in-place `out[i+j]` writes via `den_set`; three
loops — inner row-accumulate, carry-propagate, outer row-sum — with a magnitude bound keeping the
carry loop in range) · `.4` `divrem` **— in progress** (bit-serial MSB-first restoring division;
groundwork `shl1_eq` (`den = 2·den`) + `testbit_eq` (bit `i` of the denotation) landed; `bit_len`,
the division loop, and `divrem_eq` remain) · `.5` `gcd` · `.6` `RefInt`/`RefRat` → ℤ/ℚ + the
`Backend`-instance corollary.

**R.4b.4 groundwork.** `divrem` is the effort peak. Primitives, bottom-up: **`shl1_eq`** — the doubling,
`den(shl1 x) = 2·den x`, a per-limb `(v[i]<<1)|carry` loop; needs `u64_or_add` (OR = + when the low bit
is free, via `Nat.two_pow_add_eq_or_of_lt`). **`testbit_eq`** — `testbit self i = Nat.testBit (den self) i`;
the key is `den_testBit_lt` (bit `64q+r` of the limb list = bit `r` of limb `q`, via
`Nat.testBit_two_pow_mul_add`), plus `Nat.testBit_eq_decide_div_mod_eq` and `UScalar.val_and`/`eq_of_val_eq`
for the `(x>>off)&1` read. Ahead: `bit_len` (`den < 2^bit_len`; `BitVec.leadingZeros` — a real def, but the
`U32→Usize` cast of it drags in `System.Platform.numBits`, needing care; draft parked), the MSB-first
division loop (invariant `den self = den q·den d + den r·2^i + den self%2^i`, `2^i ∣ den q`, `den r < den d`;
the in-place q/r bit-sets), and `divrem_eq`.

**R.4b.3 recipe (validated).** `mul` differs from `add`/`sub`: it writes `out` in place, so the
workhorse is **`den_set`** (`den (l.set p x) = den l + (x − l[p])·2^(64p)`, over ℤ). Three
`loop.spec_decr_nat` proofs: (i) the inner `j`-loop accumulates one row `ai·v1` at offset `i`, invariant
`den out + carry·2^(64(i+j)) = den out0 + ai·den(take j v1)·2^(64i)` with `carry < 2^64`; the u128
overflow side-goals need the **tight** product bound `ai·o[j] ≤ (2^64−1)²` (a plain `< 2^128` is too weak
for the subsequent `i4 + ai·o[j]` add) so `step` can auto-discharge; the den step closes by `den_set` +
`u128_split` + a `linear_combination` in the split power basis `2^(64i)·2^(64j)` (not `2^(64·i2)` — `ring`
can't identify variable-exponent atoms). (ii) The carry `k`-loop keeps `den out + carry·2^(64k)`
invariant; in-bounds (`k < len out`) comes from a magnitude bound `value < 2^(64B)`, `B ≤ len out`:
a nonzero carry forces `2^(64k) ≤ value`, hence `k < B`. (iii) The outer `i`-loop composes the two via
`have spec … + cases hcase : loop … + WP.spec_ok`, invariant `den out = den(take i v)·den v1`, using
`den_take_succ` for the row step. `mul_eq` wraps: `isEmpty` zero-guards (no `Normalized` needed —
`[].isEmpty` gives `den = 0` directly), `from_elem 0 (n+m)` (`den_replicate_zero`), the outer loop,
`normalize_den`.

**R.4b.2 recipe (validated).** The carry loop is a `loop.spec_decr_nat` with the invariant
`den out + carry·2^(64i) = den(take i v) + den(take i v1)`, `len out = i`, `carry ≤ 1`. In the body:
`dsimp only []` zeta-reduces the `let i1 := v.len` bindings; each padded limb load is resolved to
`ok (dite …)` by `by_cases i' < v.len` + `Vec.index_slice_index`/`index_usize`, and `step` advances
the u128 arithmetic. The deep bit — the **u128 split** — is `cast_u64_val` (`s as u64 = s % 2^64`,
via `BitVec.toNat_setWidth`) + `ShiftRight` (`s >>> 64 = s / 2^64`) ⇒ `lo + 2^64·carry1 = s`
(`Nat.mod_add_div`); `cast_u128_val` (widening preserves), `den_take_succ_pad` and `den_append` close
the per-limb den bookkeeping, and the final invariant is a linear `omega` over the (mul-commuted) atoms.
`add_eq` wraps the loop with the optional final carry limb + `normalize_den`, under a `len+1 ≤ usize::MAX`
capacity precondition (vacuous for real bignums). `sub` is the dual with a signed `i128` borrow (the
`d < 0` branch), reasoned over ℤ.

R.4b.1 wired the committed extraction (`crate::refbackend` added to `lattice.startfrom`; the model
+ the external models in `Lattice/{Funs,Types}External.lean` + `CommonExtern`, drift-checked like the
other lifts) and proved the denotation core + `is_zero`/`cmp` in `CertifyCheck/RefBackend.lean`
(axiom-clean, CI-audited). Two shaping notes from R.4b.1: `RefNat::add`'s `usize::max` was rewritten
to an explicit `if` (the `Ord::max` default method mis-lifts at the pin), and the certify-core
externals were qualified to `certify_core.lattice.backend.Backend` (the `refbackend` lift adds a
concrete `Backend` to the shared model that would otherwise shadow the opaque one — see
`docs/engineering-log.md`). The `u128`-scalar reasoning in `add` prices the hardest recurring
sub-lemma; `mul` (nested) and `divrem` (bit-serial) are the effort peaks ahead.
