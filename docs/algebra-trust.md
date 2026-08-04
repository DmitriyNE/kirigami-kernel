# Algebra trust doctrine

How exact `ℤ`/`ℚ` arithmetic is trusted, proven, and kept liftable. Adopted 2026-08-04.

## The problem

Every certifier bottoms out in exact rational arithmetic. In Lean we want to prove the
*checkers* (Sturm chain-validity, resultant, CLIP-σ, …) over **real algebra** — Mathlib
`ℤ`/`ℚ` — not over an opaque bignum. But the production arithmetic is `lattice::Rat/Int`,
a two-tier `Fast(i128) | Slow(dashu)` type whose slow leaves are the external `dashu` crate
that Aeneas can't see into. So we need a discipline that (a) gives clean algebra for proofs
now, and (b) shrinks what we trust over time — without blocking on the hard part.

## The three separated concerns

1. **Algebra — Lean over `ℤ`/`ℚ` (proves the algorithms).** Checkers are lifted with
   `Int` modeled as `ℤ` and `Rat` as `ℚ` (opaque external types = their mathematical
   ideal). Every checker proof then reasons over Mathlib algebra, and the model abstracts
   the representation away. This is the clean, high-value path; it makes the `verify_chain`
   / `IsSturmChainData` class of drift (see [Sturm](proofs/sturm.md) and the false-axiom
   fix) structurally impossible, because the Lean predicate is *derived* from the lifted
   body, not hand-mirrored.

2. **Representation — Kani (proves the implementation).** The two-tier
   `Fast(i128)/Slow(bignum)` bridge — `fast ≡ slow` on the fast domain, panic/overflow
   freedom over full i128 — is Kani's job (bounded, exhaustive/differential over i128). The
   `Int=ℤ` model in concern 1 is sound *because* Kani owns this: the algebra proof may
   assume `Int` behaves as `ℤ`, and Kani certifies the representation actually does.

3. **Reference — shrinking the dashu trust.** Modeling `Int=ℤ` *trusts dashu* to implement
   exact arithmetic. That is one crisp, honest TCB entry. To reduce it: **(1)** write a slow,
   safe, Aeneas-liftable reference bignum — **done**: `lattice::RefBackend` (`refbackend.rs`), an
   independent `Vec<u64>`-limb sign-magnitude `Backend`; **(2)** prove it `= ℤ`/`ℚ` in Lean (no
   trusted hand-model) — the tracked deep follow-up; **(3)** differential-stress dashu against the
   reference — **done**: `rat::differential::{int,rat}_dashu_matches_ref` runs dashu ≡ `RefBackend`
   over the full i128 range. The `Backend` trait makes the reference a drop-in alternate backend.

## The linchpin invariant — the representation must never leak

For the `Int=ℤ`/`Rat=ℚ` model to be **sound**, no code that gets lifted may observe the
`Fast/Slow` representation. Concretely:

> The `Fast(i128) | Slow(bignum)` representation of `lattice::Int`/`Rat` is **private to
> `crates/lattice/src/rat.rs`** (plus its Kani equivalence proofs). Every other consumer —
> every checker, every geometry predicate — uses **only the arithmetic API**
> (`add`/`sub`/`mul`/`div`/`cmp`/`sign`/…), never a `match` on `Fast/Slow`.

Any `match Rat::Fast(..)` in a checker is a value that could not be lifted to `ℚ`
faithfully — it observes an implementation detail the `ℚ` model has erased.

- **Type-enforced** (algebra rehaul R.1): `Int`/`Rat` are opaque newtypes —
  `pub struct Int(IntRepr)` / `pub struct Rat(RatRepr)` over a *private* `IntRepr`/`RatRepr`
  in `lattice::rat`. The tier is unnameable outside that module, so a checker *cannot* `match`
  on `Fast/Slow` — it is a compile error, not a lint hit. This supersedes the former
  `no-repr-leak` grep lint (retired): the guarantee is now carried by the type system, which
  the earlier lint could only approximate over text.

## Status

- The two-tier is **benchmark-justified** (keep it) — see [two-tier-benchmark.md](two-tier-benchmark.md).
- The **algebra-trust rehaul** (post-B) lands in pillars, orthogonal to the two-tier (which it
  abstracts away):
  - **R.1–R.3 (merged):** opaque `Int`/`Rat`; the ℤ/ℚ-mapping spike (GO, no pin bump); and
    `certify1d::clip_sigma` lifted over ℚ with *both* cores derived from the extracted Rust
    (`ClipSigma.lean`, axiom-clean) — the hand-mirror spec-drift class killed for the ★ CLIP-σ row.
  - **R.4 (reference bignum) — steps (1)+(3) done:** `RefBackend` is differentially cross-checked
    against dashu over the full i128 range; the Lean proof `RefBackend = ℤ/ℚ` (step 2) is the
    tracked deep follow-up that would turn the cross-check into a proof-backed oracle.
