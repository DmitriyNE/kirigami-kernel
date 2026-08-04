-- Hand-written models of the opaque `lattice::rat` OPS + the `core::cmp` glue pulled into the
-- certify-core lift (algebra-rehaul R.3). Under the ℚ type model (`TypesExternal.lean`), each
-- `Rat` value IS a `ℚ`, so each op here is literally the matching Mathlib ℚ operation — the
-- lifted `clip_sigma` therefore computes over real ℚ. These are faithful `def`s (never `axiom`,
-- guarded by `scripts/check-externals.sh`): the certify-core model's only hand-written TCB
-- surface, each one small and auditable against the cited Rust op.
import Aeneas
import Mathlib
import CertifyCore.Types
import CommonExtern
open Aeneas Aeneas.Std Result ControlFlow Error
set_option linter.dupNamespace false
set_option linter.hashCommand false
set_option linter.unusedVariables false
open certify_core

-- The `?`-operator glue on `Option` (`Try::branch`, `FromResidual::from_residual`) is shared
-- with the lattice lift, so it lives in `CommonExtern` (imported above), not duplicated here.

-- ── core::cmp glue ──────────────────────────────────────────────────────────────────────────

/-- `<Ordering as PartialEq>::eq` — structural equality on the three-valued ordering. -/
@[rust_fun "core::cmp::{core::cmp::PartialEq<core::cmp::Ordering, core::cmp::Ordering>}::eq"]
def core.cmp.Ordering.Insts.CoreCmpPartialEqOrdering.eq (a b : Ordering) : Result Bool :=
  ok (decide (a = b))

-- ── lattice::Rat ops, modelled as their Mathlib ℚ counterparts ──────────────────────────────

/-- `Rat::from_i128 v` — the rational `v/1`. Over ℚ: the integer `v` coerced. -/
@[rust_fun "lattice::rat::{lattice::rat::Rat<@B, @Clause0_Int, @Clause0_Rat>}::from_i128"]
def lattice.rat.Rat.from_i128 {B Clause0_Int Clause0_Rat : Type}
    (_inst : lattice.backend.Backend B Clause0_Int Clause0_Rat) (v : Std.I128) :
    Result (lattice.rat.Rat B Clause0_Int Clause0_Rat) :=
  ok (v.val : ℚ)

/-- `Rat::sub a b` = `a - b`. -/
@[rust_fun "lattice::rat::{lattice::rat::Rat<@B, @Clause0_Int, @Clause0_Rat>}::sub"]
def lattice.rat.Rat.sub {B Clause0_Int Clause0_Rat : Type}
    (_inst : lattice.backend.Backend B Clause0_Int Clause0_Rat)
    (a b : lattice.rat.Rat B Clause0_Int Clause0_Rat) :
    Result (lattice.rat.Rat B Clause0_Int Clause0_Rat) :=
  ok (a - b)

/-- `Rat::neg a` = `-a`. -/
@[rust_fun "lattice::rat::{lattice::rat::Rat<@B, @Clause0_Int, @Clause0_Rat>}::neg"]
def lattice.rat.Rat.neg {B Clause0_Int Clause0_Rat : Type}
    (_inst : lattice.backend.Backend B Clause0_Int Clause0_Rat)
    (a : lattice.rat.Rat B Clause0_Int Clause0_Rat) :
    Result (lattice.rat.Rat B Clause0_Int Clause0_Rat) :=
  ok (-a)

/-- `Rat::sign a` = `-1 | 0 | 1` as `i8`. -/
@[rust_fun "lattice::rat::{lattice::rat::Rat<@B, @Clause0_Int, @Clause0_Rat>}::sign"]
def lattice.rat.Rat.sign {B Clause0_Int Clause0_Rat : Type}
    (_inst : lattice.backend.Backend B Clause0_Int Clause0_Rat)
    (a : lattice.rat.Rat B Clause0_Int Clause0_Rat) :
    Result Std.I8 :=
  if 0 < a then ok 1#i8
  else if a < 0 then ok (Std.I8.ofIntCore (-1) (by constructor <;> scalar_tac))
  else ok 0#i8

/-- `<Rat as Clone>::clone a` = `a` (ℚ is a value; cloning is the identity). -/
@[rust_fun "lattice::rat::{core::clone::Clone<lattice::rat::Rat<@B, @Clause0_Int, @Clause0_Rat>>}::clone"]
def lattice.rat.Rat.Insts.CoreCloneClone.clone {B Clause0_Int Clause0_Rat : Type}
    (_inst : lattice.backend.Backend B Clause0_Int Clause0_Rat)
    (a : lattice.rat.Rat B Clause0_Int Clause0_Rat) :
    Result (lattice.rat.Rat B Clause0_Int Clause0_Rat) :=
  ok a

/-- `<Rat as PartialEq>::eq a b` = `a == b`. -/
@[rust_fun "lattice::rat::{core::cmp::PartialEq<lattice::rat::Rat<@B, @Clause0_Int, @Clause0_Rat>, lattice::rat::Rat<@B, @Clause0_Int, @Clause0_Rat>>}::eq"]
def lattice.rat.Rat.Insts.CoreCmpPartialEqRat.eq {B Clause0_Int Clause0_Rat : Type}
    (_inst : lattice.backend.Backend B Clause0_Int Clause0_Rat)
    (a b : lattice.rat.Rat B Clause0_Int Clause0_Rat) :
    Result Bool :=
  ok (decide (a = b))

/-- `<Rat as PartialOrd>::partial_cmp a b` = `Some (compare a b)` (ℚ's order is total). -/
@[rust_fun "lattice::rat::{core::cmp::PartialOrd<lattice::rat::Rat<@B, @Clause0_Int, @Clause0_Rat>, lattice::rat::Rat<@B, @Clause0_Int, @Clause0_Rat>>}::partial_cmp"]
def lattice.rat.Rat.Insts.CoreCmpPartialOrdRat.partial_cmp {B Clause0_Int Clause0_Rat : Type}
    (_inst : lattice.backend.Backend B Clause0_Int Clause0_Rat)
    (a b : lattice.rat.Rat B Clause0_Int Clause0_Rat) :
    Result (Option Ordering) :=
  ok (some (compare a b))

/-- `<Rat as Ord>::cmp a b` = `compare a b` — the total order CLIP-σ ranges over. -/
@[rust_fun "lattice::rat::{core::cmp::Ord<lattice::rat::Rat<@B, @Clause0_Int, @Clause0_Rat>>}::cmp"]
def lattice.rat.Rat.Insts.CoreCmpOrd.cmp {B Clause0_Int Clause0_Rat : Type}
    (_inst : lattice.backend.Backend B Clause0_Int Clause0_Rat)
    (a b : lattice.rat.Rat B Clause0_Int Clause0_Rat) :
    Result Ordering :=
  ok (compare a b)
