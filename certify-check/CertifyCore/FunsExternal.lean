-- No hand-written externals: the slice-3e link checkers (`arrange::{v_boundary,
-- link_ok,link_iso_ok}` and their closure) lift with NO core-library holes — aeneas
-- emits no `FunsExternal_Template.lean` for `certify-core` (unlike `lattice::small`).
-- So `certify-core`'s Aeneas model has an EMPTY hand-written TCB surface: its trust
-- footprint is exactly Charon + Aeneas + Lean + Mathlib, nothing bespoke.
--
-- This file exists only because the generated `Funs.lean` imports
-- `CertifyCore.FunsExternal`; keep it in sync should a future `--start-from` addition
-- pull in a core item Aeneas cannot model (then fill the hole faithfully, as
-- `Lattice/FunsExternal.lean` does).
import Aeneas
import CertifyCore.Types
open Aeneas Aeneas.Std Result ControlFlow Error
