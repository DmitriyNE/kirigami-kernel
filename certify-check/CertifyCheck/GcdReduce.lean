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
          -- No wrap: `hfit` says the shifted gcd still fits `u128`.
          scalar_tac
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

/-- The stripped part really is odd — `natTrailingZeros` takes out *every* factor of two. -/
theorem natTrailingZeros_odd_part (n : Nat) (hn : n ≠ 0) :
    ¬ 2 ∣ (n / 2 ^ natTrailingZeros n) := by
  induction n using Nat.strong_induction_on with
  | _ n ih =>
    rw [natTrailingZeros]
    split
    · omega
    · split
      · rename_i h hodd; simpa using (by omega : ¬ 2 ∣ n)
      · rename_i h hodd
        have hne : n / 2 ≠ 0 := by omega
        have hrec := ih (n / 2) (by omega) hne
        have hsplit : n / 2 ^ (natTrailingZeros (n / 2) + 1)
            = (n / 2) / 2 ^ natTrailingZeros (n / 2) := by
          rw [pow_succ, mul_comm, ← Nat.div_div_eq_div_mul]
        rw [hsplit]
        exact hrec

/-- **The strip-twos identity** — the mathematical content of OPT.3:
    `gcd(2^i·m, 2^j·n) = 2^min(i,j)·gcd(m, n)` for odd `m`, `n`.

    This is why a power-of-two operand needs no division: its odd part is `1`, so the Euclidean
    call underneath returns immediately. -/
theorem gcd_two_pow_mul (i j m n : Nat) (hm : ¬ 2 ∣ m) (hn : ¬ 2 ∣ n) :
    Nat.gcd (2 ^ i * m) (2 ^ j * n) = 2 ^ min i j * Nat.gcd m n := by
  have hcop : ∀ (k p : Nat), ¬ 2 ∣ p → Nat.Coprime (2 ^ k) p := fun k p hp =>
    Nat.Coprime.pow_left _ ((Nat.prime_two.coprime_iff_not_dvd).mpr hp)
  rcases Nat.le_total i j with hij | hij
  · have hj : 2 ^ j = 2 ^ i * 2 ^ (j - i) := by
      rw [← pow_add]; congr 1; omega
    rw [hj, min_eq_left hij, mul_assoc, Nat.gcd_mul_left,
        Nat.Coprime.gcd_mul_left_cancel_right n (hcop (j - i) m hm)]
  · have hi : 2 ^ i = 2 ^ j * 2 ^ (i - j) := by
      rw [← pow_add]; congr 1; omega
    rw [hi, min_eq_right hij, mul_assoc, Nat.gcd_mul_left,
        Nat.gcd_comm (2 ^ (i - j) * m) n,
        Nat.Coprime.gcd_mul_left_cancel_right m (hcop (i - j) n hn), Nat.gcd_comm n m]

/-- A nonzero `u128`'s trailing-zero count is below the width — needed for the shift's side
    condition. Same argument as the `u32` fit in `Lattice/FunsExternal.lean`. -/
theorem natTrailingZeros_lt_128 (n : Nat) (hn : n ≠ 0) (hlt : n < 2 ^ 128) :
    natTrailingZeros n < 128 := by
  by_contra hcon
  push_neg at hcon
  have hle : 2 ^ natTrailingZeros n ≤ n :=
    Nat.le_of_dvd (Nat.pos_of_ne_zero hn) (two_pow_natTrailingZeros_dvd n)
  have : (2:Nat) ^ 128 ≤ 2 ^ natTrailingZeros n := Nat.pow_le_pow_right (by norm_num) hcon
  omega

/-- The `step` spec for the hand-written `trailing_zeros` model: on a nonzero input it is
    `natTrailingZeros`. -/
@[step]
theorem trailing_zeros_spec (x : Std.U128) (hx : x.val ≠ 0) :
    core.num.U128.trailing_zeros x ⦃ r => r.val = natTrailingZeros x.val ⦄ := by
  unfold core.num.U128.trailing_zeros
  rw [WP.spec_ok]
  simp [hx]

/-- The shared tail of `gcd_u128`'s main branch: strip both operands, run the loop, restore the
    common power of two. Factored out because the `shift` selection has two symmetric arms. -/
theorem gcd_u128_tail (a b : Std.U128) (ia ib shift : Std.U32)
    (ha0 : a.val ≠ 0) (hb0 : b.val ≠ 0)
    (hia : ia.val = natTrailingZeros a.val) (hib : ib.val = natTrailingZeros b.val)
    (hshift : shift.val = min ia.val ib.val) :
    (do let x ← a >>> ia
        let y ← b >>> ib
        lattice.small.gcd_u128_loop0 shift x y) ⦃ r => r.val = Nat.gcd a.val b.val ⦄ := by
  have hialt : ia.val < 128 := by
    rw [hia]; exact natTrailingZeros_lt_128 _ ha0 (by scalar_tac)
  have hiblt : ib.val < 128 := by
    rw [hib]; exact natTrailingZeros_lt_128 _ hb0 (by scalar_tac)
  step as ⟨x, hx⟩
  step as ⟨y, hy⟩
  -- `>>> i` on `Nat` is division by `2 ^ i`, so `x`/`y` are exactly the odd parts.
  have hxv : x.val = a.val / 2 ^ ia.val := by rw [hx, Nat.shiftRight_eq_div_pow]
  have hyv : y.val = b.val / 2 ^ ib.val := by rw [hy, Nat.shiftRight_eq_div_pow]
  have hax : a.val = 2 ^ ia.val * x.val := by
    rw [hxv, hia]
    exact (Nat.mul_div_cancel' (two_pow_natTrailingZeros_dvd a.val)).symm
  have hby : b.val = 2 ^ ib.val * y.val := by
    rw [hyv, hib]
    exact (Nat.mul_div_cancel' (two_pow_natTrailingZeros_dvd b.val)).symm
  have hxodd : ¬ 2 ∣ x.val := by rw [hxv, hia]; exact natTrailingZeros_odd_part _ ha0
  have hyodd : ¬ 2 ∣ y.val := by rw [hyv, hib]; exact natTrailingZeros_odd_part _ hb0
  -- The identity: the loop's answer, re-multiplied by the stripped twos, IS `gcd a b`.
  have hkey : Nat.gcd x.val y.val * 2 ^ shift.val = Nat.gcd a.val b.val := by
    rw [hax, hby, gcd_two_pow_mul _ _ _ _ hxodd hyodd, hshift, mul_comm]
  have hfit : Nat.gcd x.val y.val * 2 ^ shift.val < 2 ^ 128 := by
    rw [hkey]
    have hle : Nat.gcd a.val b.val ≤ a.val :=
      Nat.le_of_dvd (Nat.pos_of_ne_zero ha0) (Nat.gcd_dvd_left _ _)
    exact Nat.lt_of_le_of_lt hle a.hBounds
  have hsh : shift.val < 128 := by omega
  rw [← hkey]
  exact gcd_u128_loop0_spec shift x y hsh hfit

/-- **The reshaped `gcd_u128` still computes `Nat.gcd`** — the OPT.3 replacement for the original
    `gcd_u128_spec`. Zero cases are immediate; otherwise the common power of two comes out by
    `gcd_two_pow_mul` and the odd parts go through the loop proven above. -/
theorem gcd_u128_spec (a b : Std.U128) :
    lattice.small.gcd_u128 a b ⦃ r => r.val = Nat.gcd a.val b.val ⦄ := by
  unfold lattice.small.gcd_u128
  split
  · -- `a = 0`: `gcd 0 b = b`. (Avoid `simp` here — normalising the 39-digit `u128` literal
    -- blows `maxRecDepth`, the same hazard `i128FitBound` is sealed against.)
    rename_i ha
    rw [WP.spec_ok]
    have : a.val = 0 := by scalar_tac
    rw [this, Nat.gcd_zero_left]
  · split
    · rename_i hb
      rw [WP.spec_ok]
      have : b.val = 0 := by scalar_tac
      rw [this, Nat.gcd_zero_right]
    · rename_i hb0' ha0'
      have ha0 : a.val ≠ 0 := by scalar_tac
      have hb0 : b.val ≠ 0 := by scalar_tac
      step as ⟨ia, hia⟩
      step as ⟨ib, hib⟩
      -- `shift = min ia ib`, spelled as a branch in the Rust; both arms are the same tail.
      split
      · exact gcd_u128_tail a b ia ib ia ha0 hb0 hia hib (by scalar_tac)
      · exact gcd_u128_tail a b ia ib ib ha0 hb0 hia hib (by scalar_tac)

end CertifyCheck
