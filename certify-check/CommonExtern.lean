-- Shared faithful models of `core`-library items that MORE THAN ONE crate lift externalises,
-- so they are defined exactly once here rather than duplicated per crate (which would collide
-- when a proof imports both `Lattice.FunsExternal` and `CertifyCore.FunsExternal`). Both crates'
-- `FunsExternal.lean` import this. Faithful `def`s only (no `axiom`) — guarded, like the
-- per-crate models, by `scripts/check-externals.sh`.
--
-- Currently: the `?`-operator glue on `Option` (`Try::branch`, `FromResidual::from_residual`),
-- shared by `lattice::small::reduce` and `certify1d::corner_range`.
import Aeneas
open Aeneas Aeneas.Std Result ControlFlow Error
set_option linter.dupNamespace false
set_option linter.hashCommand false
set_option linter.unusedVariables false

/-- `<Option<T> as Try>::branch` — the `?`-operator split on an `Option`. -/
@[rust_fun "core::option::{core::ops::try_trait::Try<core::option::Option<@T>>}::branch"]
def core.option.Option.Insts.CoreOpsTry_traitTry.branch {T : Type} (o : Option T) :
    Result (core.ops.control_flow.ControlFlow (Option core.convert.Infallible) T) :=
  ok (match o with
      | some x => .Continue x
      | none => .Break none)

/-- `<Option<T> as FromResidual<Option<Infallible>>>::from_residual` — a `None` residual is
    always `None` (`Infallible` is uninhabited). -/
@[rust_fun "core::option::{core::ops::try_trait::FromResidual<core::option::Option<@T>, core::option::Option<core::convert::Infallible>>}::from_residual"]
def core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionInfallible.from_residual
    (T : Type) (_r : Option core.convert.Infallible) : Result (Option T) :=
  ok none
