-- Hand-written faithful Lean models for the `core`-library TYPES that Aeneas externalises on the
-- `lattice` lift path (the generated `TypesExternal_Template.lean` holes). Faithful `def`s only
-- (never `axiom`, guarded by `scripts/check-externals.sh`) so the model's `#print axioms` stays
-- clean. This is part of the lift's hand-written TCB surface, alongside `FunsExternal.lean`.
import Aeneas
open Aeneas Aeneas.Std Result ControlFlow Error
set_option linter.dupNamespace false
set_option linter.hashCommand false
set_option linter.unusedVariables false

/-- `core::mem::MaybeUninit<T>` — storage that may hold an uninitialised `T`. Charon emits it as an
    opaque type from a `Vec`-growth internal, but the reachable `refbackend` model never references
    it (0 uses in `Funs.lean`); modelled as `T` — an *initialised* cell is exactly a `T` — purely to
    keep the externals-coverage check green. Inert: no proof reasons through it. -/
@[rust_type "core::mem::maybe_uninit::MaybeUninit"]
def core.mem.maybe_uninit.MaybeUninit (T : Type) : Type := T
