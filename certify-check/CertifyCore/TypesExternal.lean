-- Hand-written model of the opaque `lattice::rat` TYPES pulled into the certify-core lift
-- (algebra-rehaul R.3). `certify-core.opaque` holds `lattice::rat` opaque, so its two-tier body
-- — which bottoms out in the external dashu bignum, unliftable by the pinned Aeneas — is NOT
-- translated; instead the opaque `Rat` is bound here to its Mathlib ideal `ℚ`, so a lifted
-- `Rat`-using checker's predicate reasons over real ℚ rather than a bignum model. This is the
-- type-level twin of the FunsExternal mechanism (docs/algebra-trust.md; R.2 spike report).
--
-- Never overwritten by `scripts/extract.sh`; its faithfulness is guarded by
-- `scripts/check-externals.sh` (no `axiom`; every opaque type has a model here).
import Aeneas
open Aeneas Aeneas.Std Result ControlFlow Error
set_option linter.dupNamespace false

/-- The opaque `lattice::Rat` ↦ Mathlib `ℚ`, dropping the vestigial backend + associated-type
    parameters (`B`, `Clause0_Int`, `Clause0_Rat`). `abbrev` (fully reducible) so instance
    resolution sees through it to `ℚ`'s ordered-field structure. -/
@[rust_type "lattice::rat::Rat"]
abbrev lattice.rat.Rat (_B _Clause0_Int _Clause0_Rat : Type) : Type := _root_.Rat
