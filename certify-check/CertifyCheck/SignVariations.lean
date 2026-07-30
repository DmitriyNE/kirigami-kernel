/-
  Sign-variation counter — the §7 spike's representative `certify-core` checker.

  Two Lean objects live here:

  * `signVariations` — the **mathematical definition** (drop the zeros, then count
    adjacent sign changes). This is the hand-written Lean spec (`vv-guide §4`): it
    *is* the formalization of what the counter means.
  * `signVariationsImp` — a faithful transcription of the streaming Rust algorithm
    in `crates/lattice/src/sturm.rs::sign_variations` (a single `last`/`v`
    accumulator pass). The hax/Aeneas-lifted model is proven equal to *this*, and
    this is proven equal to `signVariations` — so the lifted Rust meets the spec.

  Core Lean only (no Mathlib): the counter is pure list recursion, so Phase 1 of
  the spike does not depend on the Mathlib olean cache. Mathlib enters at Phase 2
  (the Sturm checker, which needs `Polynomial`).
-/

namespace CertifyCheck

/-- Count adjacent differing entries of a list (its number of sign changes).
    Structural recursion on the tail. -/
def countSignChanges : List Int → Nat
  | []            => 0
  | [_]           => 0
  | a :: b :: rest => (if a ≠ b then 1 else 0) + countSignChanges (b :: rest)

/-- **The spec.** Sign variations of a sign sequence: discard zeros, then count
    the sign changes among what remains. "Variation count = the mathematical
    definition" (`vv-guide §7`, step 3). -/
def signVariations (l : List Int) : Nat :=
  countSignChanges (l.filter (· != 0))

/-- Streaming counter, transcribed from the Rust: `last` is the most recent
    nonzero sign (`0` = "none yet"); a change is counted when a new nonzero sign
    differs from a nonzero `last`. -/
def svAux (last : Int) : List Int → Nat
  | []        => 0
  | s :: rest =>
      if s = 0 then svAux last rest
      else (if last ≠ 0 ∧ s ≠ last then 1 else 0) + svAux s rest

/-- The Rust entry point: start with the `0` sentinel. -/
def signVariationsImp (l : List Int) : Nat := svAux 0 l

/-- Key lemma (the one real proof): the streaming pass with accumulator `last`
    equals counting sign changes of the surviving nonzeros, with `last` prepended
    exactly when it is itself a nonzero sign. Generalizing `last` is what makes
    the induction go through. -/
theorem svAux_eq (last : Int) (l : List Int) :
    svAux last l
      = if last ≠ 0 then countSignChanges (last :: l.filter (· != 0))
        else countSignChanges (l.filter (· != 0)) := by
  induction l generalizing last with
  | nil =>
      -- svAux last [] = 0; both `if` branches are countSignChanges of a
      -- ≤1-element list, i.e. 0.
      simp only [svAux, List.filter_nil]
      split <;> simp [countSignChanges]
  | cons s rest ih =>
      -- Bool test `(s != 0)` vs the Prop `s = 0`: bridge once, each direction.
      by_cases hs : s = 0
      · -- s = 0: filtered out, and svAux skips it
        have hb : (s != 0) = false := by simp [hs]
        rw [svAux, if_pos hs]
        rw [show (s :: rest).filter (· != 0) = rest.filter (· != 0) from by
              simp [hb]]
        exact ih last
      · -- s ≠ 0: survives the filter
        have hb : (s != 0) = true := by simp [bne_iff_ne, hs]
        have hfil : (s :: rest).filter (· != 0) = s :: rest.filter (· != 0) := by
          simp [hb]
        have ihs : svAux s rest = countSignChanges (s :: rest.filter (· != 0)) := by
          have h := ih s; rw [if_pos hs] at h; exact h
        rw [svAux, if_neg hs, ihs]
        by_cases hl : last ≠ 0
        · -- previous sign present: one change iff s ≠ last, then continue from s
          rw [if_pos hl, hfil, countSignChanges]
          by_cases he : s = last
          · simp [he]
          · have hla : last ≠ s := fun h => he h.symm
            simp [hl, he, hla]
        · -- no previous sign: no change recorded, continue from s
          have hz : last = 0 := by omega
          subst hz
          rw [if_neg hl, hfil]
          simp

/-- **Spec theorem.** The streaming Rust algorithm computes the mathematical
    sign-variation count. -/
theorem signVariationsImp_eq_signVariations (l : List Int) :
    signVariationsImp l = signVariations l := by
  simpa [signVariationsImp, signVariations] using svAux_eq 0 l

-- Sanity checks mirroring the Rust unit tests (`sturm.rs::sign_variations_basic`).
example : signVariations [1, 1, 1] = 0 := by decide
example : signVariations [1, -1, 1] = 2 := by decide
example : signVariations [1, 0, -1, 0, 1] = 2 := by decide
example : signVariations ([] : List Int) = 0 := by decide
example : signVariationsImp [1, 0, -1, 0, 1] = 2 := by decide

end CertifyCheck
