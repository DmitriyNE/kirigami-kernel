-- Hand-written faithful Lean models for the `core`/`alloc`-library functions on the
-- `lattice` lift paths (`small::reduce` + the `refbackend` reference bignum) that Aeneas's
-- Std library does not (yet) model.
--
-- Aeneas machine-translates the Rust into `Funs.lean`, but a handful of `core` conversions,
-- `alloc::vec` methods, and `?`-operator glue bottom out in functions its Std library lacks
-- builtin models for; it emits them as holes in the generated `FunsExternal_Template.lean`.
-- Rather than leave them as `axiom`s — which would pollute every downstream proof's
-- `#print axioms` footprint and defeat the axiom-clean guarantee — we fill each hole with a
-- *faithful* definition mirroring the documented Rust semantics of the corresponding function.
--
-- These `def`s (+ `CommonExtern`'s shared glue + `TypesExternal`'s `MaybeUninit`) are the ENTIRE
-- hand-written TCB surface of the `lattice` model beyond Aeneas/Charon/Lean/Mathlib.  Each is
-- small and directly auditable against the Rust reference cited in its doc-comment.
--   * `small::reduce` path: `unsigned_abs`, `try_from`, `Result.ok` (+ `?`-glue in `CommonExtern`).
--   * `small` off-path (`neg`/`sign` only): `checked_neg`, `signum`.
--   * `refbackend` path (algebra-rehaul R.4b): `Vec::is_empty` (on the `is_zero`/`cmp`/`normalize`
--     proof paths), `Vec::pop` (`normalize`), and — reached only by later-phase ops — `wrapping_neg`
--     and `usize::div_ceil` (+ `<Ordering as PartialEq>::eq` in `CommonExtern`).  Modelled
--     faithfully so the whole `lattice` model stays axiom-free.
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

-- ── refbackend externals (algebra-rehaul R.4b) ──────────────────────────────────────────────

/-- `<i128>::wrapping_neg` — two's-complement negation that wraps: `-x` for every `x` except
    `i128::MIN`, whose negation overflows and wraps back to `i128::MIN`.  The `else` branch is
    reached exactly at `x = MIN` (`-x.val ≥ 2^127 ⇒ x.val ≤ -2^127 ⇒ x.val = MIN`), where
    `wrapping_neg(MIN) = MIN = x`.  Reached only by later-phase sign handling. -/
@[rust_fun "core::num::{i128}::wrapping_neg"]
def core.num.I128.wrapping_neg (x : Std.I128) : Result Std.I128 :=
  if h : (-x.val) < i128FitBound
  then ok (Std.I128.ofIntCore (-x.val)
    (by rw [i128FitBound_def] at h; constructor <;> scalar_tac))
  else ok x

/-- `<usize>::div_ceil a b` — the ceiling division `⌈a/b⌉ = (a + b − 1) / b` (for `b > 0`; Rust
    panics at `b = 0`, unreachable here — the sole call site is `bit_len / 64`).  The result is
    `≤ a ≤ usize::MAX`, so it always fits.  Reached only by `divrem`'s `bit_len` (later phase). -/
@[rust_fun "core::num::{usize}::div_ceil"]
def core.num.Usize.div_ceil (a b : Std.Usize) : Result Std.Usize :=
  ok (Std.Usize.ofNatCore ((a.val + b.val - 1) / b.val) (by
    rcases Nat.eq_zero_or_pos b.val with hb | hb
    · simp [hb]
    · have h1 : (a.val + b.val - 1) / b.val ≤ a.val := by
        rw [Nat.div_le_iff_le_mul_add_pred hb]
        have := Nat.le_mul_of_pos_left a.val hb
        omega
      have := a.hBounds
      scalar_tac))

/-- `<Vec<T>>::is_empty` — `true` iff the vector has no elements (`len() == 0`), i.e. its list
    model is `[]`.  On the `is_zero` / `cmp` / `normalize` proof paths. -/
@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::is_empty"]
def alloc.vec.Vec.is_empty {T : Type} (_A : Type) (v : alloc.vec.Vec T) : Result Bool :=
  ok v.val.isEmpty

/-- `<Vec<T>>::pop` — remove and return the last element (`None` if empty), leaving the prefix.
    Aeneas models the `&mut self` as the returned new vector: `(popped?, self.dropLast)`.  On the
    `normalize` path (drops trailing zero limbs). -/
@[rust_fun "alloc::vec::{alloc::vec::Vec<@T>}::pop"]
def alloc.vec.Vec.pop {T : Type} (_A : Type) (v : alloc.vec.Vec T) :
    Result (Option T × alloc.vec.Vec T) :=
  ok (v.val.getLast?, ⟨v.val.dropLast, by
    have h := v.property
    rw [List.length_dropLast]
    omega⟩)
