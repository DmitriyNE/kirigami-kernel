# Trusted invariants — pure-tier assertions discharged by argument

The pure tier (`lattice`, `certify-core`) is **panic-free**, enforced mechanically by
`#![cfg_attr(not(test), deny(clippy::{unwrap_used, expect_used, panic, unreachable, todo,
unimplemented}))]` on each crate root, plus the `cargo xtask lint` **PANIC-FREEDOM meta-check**:
every `#[allow(clippy::…)]` in the pure tier must carry a `// PANIC-FREEDOM:` tag, and this
ledger is where those tags point.

Almost every would-be panic is *hardened away* — rewritten total, returning a defined value on
the (dead) `None`/error branch. This ledger lists the **residue**: assertions we deliberately
keep as **fail-fast** (a silent fallback would *mask* a real defect instead of surfacing it) and
discharge by a **structural argument** rather than a machine proof. Each is a **trust anchor to
revisit** — ideally to replace the argument with a machine-checked proof, at which point it
leaves this list.

Status: 🧠 discharged by argument (pending machine proof) · ✅ machine-proved.

Related: [`docs/proofs/ledger.md`](proofs/ledger.md) (the theorems these arguments cite) ·
[`docs/engineering-log.md`](engineering-log.md) (general todos/debt) · the Kani/Lean surfaces.

## Entries

### 🧠 `Surd::to_algreal` — the isolating interval always exists
- **Site:** `crates/lattice/src/algebraic.rs`, `unreachable!("a+b√d must lie in one isolating interval")`.
- **Assertion:** after iterating `SturmChain::isolate_all()`, one interval contains `a + b√d`.
- **Fail-fast, not total:** there is no correct fallback `AlgReal` to return; a silent wrong
  value would mask a Sturm-isolation regression. The assert catches exactly that class.
- **Discharge:** `a + b√d` is a root of the minimal polynomial `x² − 2a·x + (a² − b²d)` built
  just above the loop; `isolate_all` returns disjoint intervals covering **every** real root of
  that polynomial (Sturm — the obligation tracked in `docs/proofs/ledger.md` /
  `docs/proofs/sturm.md`). Therefore exactly one interval contains it.
- **Revisit:** upgrade to ✅ when Sturm root-isolation completeness is machine-proved in Lean
  (the same obligation `sturm_root_count` rests on). *Not* Kani-tractable — the isolation's
  gcd/division loop is CBMC-hard (see the note in `crates/lattice/src/proof.rs`).
