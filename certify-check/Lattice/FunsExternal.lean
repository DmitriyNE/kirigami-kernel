-- Hand-written faithful Lean models for the `core`-library functions on the
-- `lattice::small` lift path that Aeneas's Std library does not (yet) model.
--
-- Aeneas machine-translates the Rust `small` module into `Funs.lean`, but a
-- handful of `core` conversions and `?`-operator glue bottom out in functions
-- its Std library lacks builtin models for; it emits them as holes in this
-- file (the generated `FunsExternal_Template.lean`).  Rather than leave them as
-- `axiom`s — which would pollute every downstream proof's `#print axioms`
-- footprint and defeat the axiom-clean guarantee — we fill each hole with a
-- *faithful* definition mirroring the documented Rust semantics of the
-- corresponding `core` function.
--
-- These are the ONLY hand-written pieces of the `small` model, and thus its
-- entire TCB surface beyond Aeneas/Charon/Lean/Mathlib.  Each is small and
-- directly auditable against the Rust reference cited in its doc-comment.
--   * On the `reduce` proof path: `unsigned_abs`, `try_from`, `Result.ok` (and the
--     `?`-operator `branch`/`from_residual`, shared with certify-core, in `CommonExtern`).
--   * Off-path (only reached by the sibling `neg`/`sign`): `checked_neg`,
--     `signum` — modelled faithfully too, so the whole `small` model stays
--     axiom-free, but not exercised by any current proof.
import Aeneas
import Lattice.Types
import CommonExtern
open Aeneas Aeneas.Std Result ControlFlow Error
set_option linter.dupNamespace false
set_option linter.hashCommand false
set_option linter.unusedVariables false
open lattice

/-- `2^127 = |i128::MIN|` — the magnitude at/above which a `u128` no longer fits
    in an `i128`.  Sealed `irreducible` so that proofs casing on the fit-check
    never force the elaborator to unary-normalise the 39-digit literal (which
    otherwise blows `maxRecDepth`); unfold it with `i128FitBound_def`. -/
irreducible_def i128FitBound : Int := 2 ^ 127

/-- `<i128>::unsigned_abs` — the magnitude `|x|` as a `u128`.
    Ref: `core::num::{i128}::unsigned_abs`.  `|x| ∈ [0, 2^127] ⊂ [0, 2^128)`. -/
@[rust_fun "core::num::{i128}::unsigned_abs"]
def core.num.I128.unsigned_abs (x : Std.I128) : Result Std.U128 :=
  ok (Std.U128.ofNatCore x.val.natAbs (by scalar_tac))

/-- `TryFrom<u128> for i128` — checked narrowing: `Ok v` iff the magnitude fits
    in `i128` (`< 2^127`), else `Err(TryFromIntError)`.
    Ref: `core::convert::num` `TryFrom<u128> for i128`. -/
@[rust_fun
  "core::convert::num::{core::convert::TryFrom<i128, u128, core::num::error::TryFromIntError>}::try_from"]
def I128.Insts.CoreConvertTryFromU128TryFromIntError.try_from (u : Std.U128) :
    Result (core.result.Result Std.I128 core.num.error.TryFromIntError) :=
  if h : (u.val : Int) < i128FitBound
  then ok (.Ok (Std.I128.ofIntCore (u.val : Int)
    (by rw [i128FitBound_def] at h; constructor <;> scalar_tac)))
  else ok (.Err ())

-- The `?`-operator glue (`Try::branch`, `FromResidual::from_residual`) is shared with the
-- certify-core lift, so it lives in `CommonExtern` (imported above), not duplicated here.

/-- `Result::ok` — discard the error, keep `Some` on success. -/
@[rust_fun "core::result::{core::result::Result<@T, @E>}::ok"]
def core.result.Result.ok {T E : Type} (r : core.result.Result T E) : Result (Option T) :=
  pure (show Option T from match r with
      | .Ok v  => some v
      | .Err _ => none)

/-- `<i128>::checked_neg` — `Some (-x)` unless `x = i128::MIN` (whose negation
    overflows), giving `None`.  Off the `reduce` path (used by `neg`). -/
@[rust_fun "core::num::{i128}::checked_neg"]
def core.num.I128.checked_neg (x : Std.I128) : Result (Option Std.I128) :=
  if h : (-x.val) < i128FitBound
  then ok (some (Std.I128.ofIntCore (-x.val)
    (by rw [i128FitBound_def] at h; constructor <;> scalar_tac)))
  else ok none

/-- `<i128>::signum` — the sign as `-1 / 0 / 1`.  Off the `reduce` path
    (used by `sign`). -/
@[rust_fun "core::num::{i128}::signum"]
def core.num.I128.signum (x : Std.I128) : Result Std.I128 :=
  if x.val > 0 then ok 1#i128
  else if x.val < 0 then ok (Std.I128.ofIntCore (-1) (by constructor <;> scalar_tac))
  else ok 0#i128
