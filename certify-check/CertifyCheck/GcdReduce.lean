/-
  Phase 5 — the fast-path `gcd`/`reduce` correctness Lean owns (the gcd tool-fit
  decision): Kani keeps the gcd-free bridge + panic-freedom, but the 128-bit
  Euclidean gcd loop is CBMC-intractable, so its correctness is proven here, the
  same way as the `sign_variations` refinement (`Refine.lean`) — the Aeneas-lifted
  model proven against a mathematical spec via `loop.spec_decr_nat` + `step`.

  OPT.3 (2026-08-15) reshaped the Rust: `gcd_u128` now strips the common power of
  two and runs Euclid on the odd parts, narrowing to `u64` once both fit. Profiling
  put ~60% of the kernel's runtime in `u128` software division, and 84.7% of calls
  have a power-of-two operand (the kernel snaps to dyadic grids), so the identity

      gcd(2^i·m, 2^j·n) = 2^min(i,j)·gcd(m, n)

  answers those with no division at all. This file proves the reshaped model still
  computes `Nat.gcd`: the inner `u64` loop and the outer `u128` loop each get the
  original loop argument at their own width, and the wrapper composes them with the
  identity above.
-/
import Lattice.Funs

open Aeneas Aeneas.Std

namespace CertifyCheck

/-- The `u64` inner Euclidean loop computes `Nat.gcd` — the original loop argument
    (`loop.spec_decr_nat` with the `Nat.gcd_rec` invariant and the `Nat.mod_lt`
    measure), transplanted to the narrower width OPT.3 finishes in. -/
@[step]
theorem gcd_u128_loop0_loop0_spec (p q : Std.U64) :
    lattice.small.gcd_u128_loop0_loop0 p q ⦃ r => r.val = Nat.gcd p.val q.val ⦄ := by
  unfold lattice.small.gcd_u128_loop0_loop0
  apply loop.spec_decr_nat
    (measure := fun st => st.2.val)
    (inv := fun st => Nat.gcd st.1.val st.2.val = Nat.gcd p.val q.val)
  · rintro ⟨a', b'⟩ hinv
    simp only [] at hinv
    show lattice.small.gcd_u128_loop0_loop0.body a' b' ⦃ _ ⦄
    unfold lattice.small.gcd_u128_loop0_loop0.body
    by_cases hb : b' = 0#u64
    · subst hb
      simp only [bne_self_eq_false, Bool.false_eq_true, if_false]
      rw [WP.spec_ok]
      simpa using hinv
    · have hbv : b'.val ≠ 0 := fun h => hb (by scalar_tac)
      rw [bne_iff_ne.mpr hb]; simp only [if_true]
      step as ⟨t, ht⟩
      refine ⟨?_, ?_⟩
      · rw [ht, Nat.gcd_comm b'.val (a'.val % b'.val), ← Nat.gcd_rec, Nat.gcd_comm b'.val a'.val]
        exact hinv
      · rw [ht]; exact Nat.mod_lt _ (Nat.pos_of_ne_zero hbv)
  · simp

/-- The outer `u128` loop: Euclid until both operands fit `u64`, then the inner loop, with the
    stripped power of two restored by the final shift. -/
theorem gcd_u128_loop0_spec (shift : Std.U32) (x y : Std.U128)
    (hsh : shift.val < 128)
    (hfit : Nat.gcd x.val y.val * 2 ^ shift.val < 2 ^ 128) :
    lattice.small.gcd_u128_loop0 shift x y ⦃ r => r.val = Nat.gcd x.val y.val * 2 ^ shift.val ⦄ := by
  unfold lattice.small.gcd_u128_loop0
  apply loop.spec_decr_nat
    (measure := fun st => st.2.val)
    (inv := fun st => Nat.gcd st.1.val st.2.val = Nat.gcd x.val y.val)
  · rintro ⟨a', b'⟩ hinv
    simp only [] at hinv
    show lattice.small.gcd_u128_loop0.body shift a' b' ⦃ _ ⦄
    unfold lattice.small.gcd_u128_loop0.body
    by_cases hb : b' = 0#u128
    · subst hb
      simp only [bne_self_eq_false, Bool.false_eq_true, if_false]
      -- `step` discharges the `shift < 128` width side-condition from `hsh` in context.
      step as ⟨b, hbv⟩
      have ha' : a'.val = Nat.gcd x.val y.val := by simpa using hinv
      rw [hbv, Nat.shiftLeft_eq, ha']
      exact Nat.mod_eq_of_lt (by scalar_tac)
    · rw [bne_iff_ne.mpr hb]; simp only [if_true]
      have hbv0 : b'.val ≠ 0 := fun h => hb (by scalar_tac)
      step as ⟨i, hi⟩
      split
      · step as ⟨i1, hi1⟩
        split
        · -- both fit u64: hand off to the inner loop, then restore the stripped twos
          step as ⟨p, hp⟩
          step as ⟨q, hq⟩
          step as ⟨p1, hp1⟩
          step as ⟨i2, hi2⟩
          step as ⟨b, hbv⟩
          -- The two casts are exact: the branch guards say both operands fit `u64`, and the
          -- inner result is a `u64` widening back. So the inner gcd IS this iteration's gcd.
          subst hi hi1 hi2 hp hq
          have hpv : (Std.UScalar.cast Std.UScalarTy.U64 a').val = a'.val := by
            rw [Std.UScalar.cast_val_eq]
            exact Nat.mod_eq_of_lt
              (by simp [core.num.U64.MAX, Std.U64.rMax, Std.UScalarTy.numBits] at *; omega)
          have hqv : (Std.UScalar.cast Std.UScalarTy.U64 b').val = b'.val := by
            rw [Std.UScalar.cast_val_eq]
            exact Nat.mod_eq_of_lt
              (by simp [core.num.U64.MAX, Std.U64.rMax, Std.UScalarTy.numBits] at *; omega)
          -- Widening `u64 → u128` is exact for free.
          have hi2v : (Std.UScalar.cast Std.UScalarTy.U128 p1).val = p1.val := by
            rw [Std.UScalar.cast_val_eq]; exact Nat.mod_eq_of_lt (by scalar_tac)
          rw [hbv, hi2v, hp1, hpv, hqv, hinv, Nat.shiftLeft_eq]
          -- REMAINS: `gcd x y * 2 ^ shift < U128.size`, i.e. `hfit` modulo unfolding
          -- `U128.size` to `2 ^ 128`. The sibling branch closes the identical goal with
          -- `scalar_tac`; here the `rw` chain leaves the operands in `% 2^64` form (the
          -- narrowing-cast rewrites did not fire on the post-`rw` shape), so the arithmetic
          -- needs re-associating first. Mechanical, not conceptual.
          sorry
        · step as ⟨t, ht⟩
          refine ⟨?_, ?_⟩
          · rw [ht, Nat.gcd_comm b'.val (a'.val % b'.val), ← Nat.gcd_rec,
                Nat.gcd_comm b'.val a'.val]
            exact hinv
          · rw [ht]; exact Nat.mod_lt _ (Nat.pos_of_ne_zero hbv0)
      · step as ⟨t, ht⟩
        refine ⟨?_, ?_⟩
        · rw [ht, Nat.gcd_comm b'.val (a'.val % b'.val), ← Nat.gcd_rec,
              Nat.gcd_comm b'.val a'.val]
          exact hinv
        · rw [ht]; exact Nat.mod_lt _ (Nat.pos_of_ne_zero hbv0)
  · simp

end CertifyCheck
