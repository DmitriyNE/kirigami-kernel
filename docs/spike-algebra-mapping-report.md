# R.2 spike — `Int=ℤ` / `Rat=ℚ` at the Aeneas boundary: report & go/no-go

*Status: **COMPLETE — decision: GO.** The algebra-trust rehaul's deep pillar (model the opaque
`lattice::Int`/`Rat` as Mathlib `ℤ`/`ℚ` at the Aeneas boundary, so lifted checkers reason over real
algebra) is viable at the **pinned** toolchain — no pin bump needed. Recorded: the exact mechanism, the
one real wrinkle R.3 must solve (the `Backend` trait's duplicate-field extraction), and the recipe.*

## 0. What the spike priced

The highest-variance unknown in the rehaul plan: **can the pinned Aeneas map a *user opaque type*
(`lattice::Int<B: Backend>`) to a *Mathlib type* (`ℤ`), dropping the trait-bounded generic, with its
methods bound to Mathlib ops — so a lifted checker computes over real ℤ/ℚ?** The repo had only ever
externalised `core`-library *functions* this way (`FunsExternal.lean`, 7 items); mapping a user *type*
to Mathlib was unexercised and the report-flagged risk was that it would force a coupled
Charon/Aeneas/Lean/Mathlib pin bump.

Pins (unchanged, confirmed working): Aeneas `3a8586fa`, Charon `0.1.225`, Lean `v4.31.0`, Mathlib
`v4.31.0`.

## 1. Result — GO, proven end-to-end and axiom-clean

Two throwaway probes (`spike_int_probe<B>(a,b) = (a+b)*b` over `Int<B>`; `spike_rat_probe<B>(a,b) = a+b`
over `Rat<B>`) were lifted **from real `crates/lattice` Rust through charon+aeneas at the pins**, the
opaque types bound to `ℤ`/`ℚ`, and the lifted models **proven** to compute the real arithmetic:

- `spike_int_probe_eq : spike_int_probe inst a b = ok ((a + b) * b)` over `ℤ` — `#print axioms` = `[propext]`.
- `spike_rat_probe_eq : spike_rat_probe inst a b = ok (a + b)` over `ℚ` — `#print axioms` =
  `[propext, Classical.choice, Quot.sound]` (the standard Mathlib-`ℚ` trio).

**No `sorryAx`** — the Aeneas Std `get_unchecked` `sorry`s are off this path. So the ℤ/ℚ-at-the-boundary
lift typechecks and lets a consumer reason over real Mathlib algebra, at the pinned toolchain.

## 2. The mechanism (the R.3 recipe, validated)

1. **Opaque the module, not per-item.** `charon cargo --preset=aeneas --start-from <checker>
   --opaque 'lattice::rat'` — the **bare-module** name-matcher (`--opaque crate::module`, "won't explore
   its contents") cleanly opaques the `Int`/`Rat` types **and** their methods in one lever. Per-method
   name-patterns (`{lattice::rat::Int<@B>}::add`) are finicky — charon's CLI name-matcher differs from
   Aeneas's `@[rust_fun]` syntax (`<@B>` fails to parse; the impl block is an anonymous `{IMPL}`). Prefer
   the module lever.
2. **Aeneas emits the stubs.** Opaque types → `TypesExternal_Template.lean`
   (`axiom rat.Int (B) (Clause0_Int) (Clause0_Rat) : Type`); opaque methods → `FunsExternal_Template.lean`
   (`axiom rat.Int.add … : rat.Int … → rat.Int … → Result (rat.Int …)`), carrying the name pattern
   `lattice::rat::{lattice::rat::Int<B, Clause0_Int, Clause0_Rat>}::add`.
3. **Assoc-type unbundling.** `Int<B: Backend>` lifts to **three** type params —
   `rat.Int B Clause0_Int Clause0_Rat` — because the trait's assoc types `B::Int`/`B::Rat` become
   explicit params (`Clause0_Int`/`Clause0_Rat`). The ℤ/ℚ binding drops all three.
4. **Bind (the `TypesExternal.lean` / `FunsExternal.lean` twin).**
   - Type: `@[rust_type "lattice::rat::Int"] abbrev rat.Int (_ _ _ : Type) := ℤ` (and `Rat := ℚ`).
     **`abbrev`, not `@[reducible] def`** — instance search must see through it to `Add ℤ`/`Add ℚ`
     (a plain reducible `def` left `HAdd (rat.Int …)` unsynthesizable).
   - Method: `@[rust_fun "…Int…::add"] def rat.Int.add (_inst) (a b) : Result … := ok (a + b)` — under the
     type model, `a b : rat.Int _ _ _` **are** `ℤ`, so `a + b` is real integer addition.

## 3. The one wrinkle R.3 must solve — the `Backend` trait dup-field extraction

Naively lifting the `Backend` trait (needed as the vestigial `[Backend B]` instance param) produces a
Lean `structure backend.Backend` with **duplicate field names**: `type Int: Clone + Eq` and
`type Rat: Clone + Eq` each contribute a `corecloneCloneInst` and a `corecmpEqInst`, and Lean rejects the
second (`Field 'corecloneCloneInst' has already been declared`). The spike dedup'd by hand to finish the
proof; R.3 needs a real fix. Candidates, cheapest first:

- **(recommended, to validate) Opaque `lattice::backend` too.** Checkers never call a `Backend` method
  *directly* — they call `Rat`/`Int` methods (whose opaque bodies hide the `Backend` calls). So making
  `Backend` an opaque type (no structure, no fields) and leaving `[Backend B]` a vestigial opaque param
  should sidestep the collision entirely, and matches the "opaque the whole arithmetic surface" posture.
- Charon flags around trait supertrait-bounds / associated-type transforms (`--hide-marker-traits`, the
  assoc-type→param transform) — investigate if the opaque route has a snag.
- Post-process rename — a last resort (fights the drift-check's byte-determinism).

This is trait *plumbing*, orthogonal to the ℤ/ℚ mapping, which is settled GO.

## 4. Standing notes

- Toolchain builds/runs native on aarch64-darwin from source (no binary cache; one-time ~min build,
  now warm). Darwin: **unset `DEVELOPER_DIR`/`SDKROOT`** for charon/aeneas/lake (the §7 `xcrun` quirk).
- The spike touched nothing committed: probe in a throwaway `mod spike`, extraction into temp dirs, the
  Lean proof in a scratch `certify-check/Spike` lib — all reverted. The committed `Lattice`/`CertifyCore`
  models are untouched.
