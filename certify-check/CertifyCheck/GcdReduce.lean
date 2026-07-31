/-
  Phase 5 — the fast-path `gcd`/`reduce` correctness Lean owns (the gcd tool-fit
  decision): Kani keeps the gcd-free bridge + panic-freedom, but the 128-bit
  Euclidean gcd loop is CBMC-intractable, so its correctness is proven here, the
  same way as the `sign_variations` refinement (`Refine.lean`) — the Aeneas-lifted
  model proven against a mathematical spec via `loop.spec_decr_nat` + `step`.
-/
import Lattice.Funs

open Aeneas Aeneas.Std

namespace CertifyCheck

/-- The Aeneas-lifted fast-path `gcd_u128` (the `u128` Euclidean loop) computes
    `Nat.gcd`. -/
theorem gcd_u128_spec (a b : Std.U128) :
    lattice.small.gcd_u128 a b ⦃ r => r.val = Nat.gcd a.val b.val ⦄ := by
  unfold lattice.small.gcd_u128 lattice.small.gcd_u128_loop
  apply loop.spec_decr_nat
    (measure := fun st => st.2.val)
    (inv := fun st => Nat.gcd st.1.val st.2.val = Nat.gcd a.val b.val)
  · rintro ⟨a', b'⟩ hinv
    simp only [] at hinv
    show lattice.small.gcd_u128_loop.body a' b' ⦃ _ ⦄
    unfold lattice.small.gcd_u128_loop.body
    by_cases hb : b' = 0#u128
    · -- b' = 0: return a'
      subst hb
      simp only [bne_self_eq_false, Bool.false_eq_true, if_false]
      rw [WP.spec_ok]
      simpa using hinv
    · -- b' ≠ 0: continue with (b', a' % b')
      have hbv : b'.val ≠ 0 := fun h => hb (by scalar_tac)
      rw [bne_iff_ne.mpr hb]; simp only [if_true]
      step as ⟨t, ht⟩
      refine ⟨?_, ?_⟩
      · -- gcd invariant preserved: gcd b (a % b) = gcd a b
        rw [ht, Nat.gcd_comm b'.val (a'.val % b'.val), ← Nat.gcd_rec, Nat.gcd_comm b'.val a'.val]
        exact hinv
      · -- measure decreases
        rw [ht]; exact Nat.mod_lt _ (Nat.pos_of_ne_zero hbv)
  · -- initial state
    simp

end CertifyCheck
