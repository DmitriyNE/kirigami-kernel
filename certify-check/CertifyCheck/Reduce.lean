/-
  Phase 5b — the fast-path `SmallRat::reduce` correctness Lean owns (the gcd
  tool-fit decision).  The Aeneas-lifted `reduce` (built on the CBMC-intractable
  128-bit `gcd_u128`, proven in `GcdReduce.lean`) is proven to produce the
  canonical reduced form of `num/den`: a positive-denominator, coprime rational
  equal to `num/den`.  Same idiom as `Refine.lean`/`GcdReduce.lean` (the
  Aeneas-lifted model against a mathematical spec), plus the faithful Std-gap
  models in `Lattice/FunsExternal.lean` for the `core` conversions/`?`-glue.
-/
import Lattice.Funs
import CertifyCheck.GcdReduce

open Aeneas Aeneas.Std

set_option maxRecDepth 8192

namespace CertifyCheck

/-- `reduce num den` is *correct* when it returns `some sr`: `sr` is the
    canonical reduced form of `num/den` — positive denominator, coprime, and
    equal to `num/den` (cross-multiplied). -/
def ReduceOk (num den : Std.I128) (sr : lattice.small.SmallRat) : Prop :=
  0 < sr.den.val ∧
  Nat.gcd sr.num.val.natAbs sr.den.val.natAbs = 1 ∧
  sr.num.val * den.val = num.val * sr.den.val

/-! ### Faithful-model specs for the `FunsExternal` Std-gap stubs -/

@[step]
theorem unsigned_abs_spec (x : Std.I128) :
    core.num.I128.unsigned_abs x ⦃ r => r.val = (x.val.natAbs : Nat) ⦄ := by
  unfold core.num.I128.unsigned_abs
  simp only [WP.spec_ok]
  exact U128.ofNatCore_val_eq _

@[step]
theorem try_from_spec (u : Std.U128) :
    I128.Insts.CoreConvertTryFromU128TryFromIntError.try_from u
    ⦃ r => if (u.val : Int) < i128FitBound
           then ∃ v : Std.I128, r = .Ok v ∧ v.val = (u.val : Int)
           else r = .Err () ⦄ := by
  unfold I128.Insts.CoreConvertTryFromU128TryFromIntError.try_from
  by_cases h : (u.val : Int) < i128FitBound
  · rw [dif_pos h, WP.spec_ok, if_pos h]
    exact ⟨_, rfl, IScalar.ofInt_val_eq _⟩
  · rw [dif_neg h, WP.spec_ok, if_neg h]

@[step]
theorem neg_mag_spec (m : Std.U128) :
    lattice.small.neg_mag m
    ⦃ o => if (m.val : Int) ≤ i128FitBound
           then ∃ v : Std.I128, o = some v ∧ v.val = -(m.val : Int)
           else o = none ⦄ := by
  unfold lattice.small.neg_mag lattice.small.neg_mag.LIM
  step as ⟨i, hi⟩
  have hival : i.val = 2 ^ 127 - 1 := by rw [hi]; simp [I128.rMax]
  have hovf : i.val + (1#u128).val ≤ U128.max := by
    simp only [hival]; simp [U128.max, U128.numBits]
  step as ⟨lim, hlim⟩
  have hlimval : lim.val = 2 ^ 127 := by
    rw [hlim, hival]; rw [Nat.sub_add_cancel Nat.one_le_two_pow]
  have hMIN : core.num.I128.MIN.val = -(2 ^ 127) := by simp [I128.rMin]
  unfold core.cmp.impls.OrdU128.cmp
  simp only [lift]
  rcases lt_trichotomy m.val lim.val with hlt | heq | hgt
  · -- m < 2^127: `neg_mag` returns `some (-(m))`
    have hm127 : m.val < 2 ^ 127 := by omega
    rw [Nat.compare_eq_lt.mpr hlt]
    have hcv : (UScalar.hcast IScalarTy.I128 m).val = (m.val : Int) := by
      simp only [UScalar.hcast_val_eq]
      apply Aeneas.Arith.Int.bmod_pow2_eq_of_inBounds' _ _ (IScalarTy.numBits_nonzero _)
      · have h1 : (0 : Int) ≤ 2 ^ (IScalarTy.I128.numBits - 1) := by positivity
        have h2 : (0 : Int) ≤ (m.val : Int) := Int.natCast_nonneg _
        omega
      · show (m.val : Int) < 2 ^ (IScalarTy.I128.numBits - 1)
        have hnb : IScalarTy.I128.numBits - 1 = 127 := by rfl
        rw [hnb]; exact_mod_cast hm127
    have hne : UScalar.hcast IScalarTy.I128 m ≠ IScalar.min .I128 := by
      simp only [ne_eq, hcv, IScalar.min_IScalarTy_I128_eq, I128.min_eq]; omega
    step as ⟨i2, hi2⟩
    rw [if_pos (by rw [i128FitBound_def]; omega)]
    exact ⟨i2, rfl, by rw [hi2, hcv]⟩
  · -- m = 2^127: `neg_mag` returns `some i128::MIN = some (-(2^127))`
    rw [Nat.compare_eq_eq.mpr heq]
    refine (WP.spec_ok _).mpr ?_
    rw [if_pos (by rw [i128FitBound_def]; omega)]
    exact ⟨core.num.I128.MIN, rfl, by rw [hMIN]; omega⟩
  · -- m > 2^127: `neg_mag` returns `none`
    rw [Nat.compare_eq_gt.mpr hgt]
    refine (WP.spec_ok _).mpr ?_
    rw [if_neg (by rw [i128FitBound_def]; omega)]

/-! ### `step` specs for the pure `?`-operator glue -/

@[step]
theorem branch_step {T : Type} (o : Option T) :
    core.option.Option.Insts.CoreOpsTry_traitTry.branch o
    ⦃ cf => cf = match o with | some x => .Continue x | none => .Break none ⦄ := by
  unfold core.option.Option.Insts.CoreOpsTry_traitTry.branch
  exact (WP.spec_ok _).mpr rfl

@[step]
theorem resultOk_step {T E : Type} (r : core.result.Result T E) :
    core.result.Result.ok r
    ⦃ o => o = match r with | .Ok v => some v | .Err _ => none ⦄ := by
  unfold core.result.Result.ok
  exact (WP.spec_ok _).mpr rfl

@[step]
theorem fromResidual_step {T : Type} (r : Option core.convert.Infallible) :
    core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionInfallible.from_residual T r
    ⦃ o => o = none ⦄ := by
  unfold core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionInfallible.from_residual
  exact (WP.spec_ok _).mpr rfl

/-! ### The refinement: `reduce` computes the canonical reduced rational -/

attribute [local step] gcd_u128_spec

/-- **The refinement.**  Whenever the Aeneas-lifted `SmallRat::reduce` returns
    `some sr`, `sr` is the canonical reduced form of `num/den`: positive
    denominator, coprime numerator/denominator, and equal to `num/den`. -/
theorem reduce_spec (num den : Std.I128) :
    lattice.small.SmallRat.reduce num den
    ⦃ r => ∀ sr, r = some sr → ReduceOk num den sr ⦄ := by
  unfold lattice.small.SmallRat.reduce
  by_cases hden0 : den = 0#i128
  · rw [if_pos hden0]
    refine (WP.spec_ok _).mpr ?_
    intro sr hsr; simp at hsr
  · rw [if_neg hden0]
    have hdenv : den.val ≠ 0 := fun h => hden0 (by scalar_tac)
    step as ⟨i, hi⟩
    step as ⟨i1, hi1⟩
    step as ⟨g, hg⟩
    have hgpos : 0 < g.val := by
      rw [hg]
      apply Nat.pos_of_ne_zero
      intro h0
      rw [Nat.gcd_eq_zero_iff] at h0
      rw [hi1] at h0
      exact hdenv (Int.natAbs_eq_zero.mp h0.2)
    step as ⟨n_mag, hnm⟩
    step as ⟨d_mag, hdm⟩
    -- shared facts about the reduced magnitudes
    have hgdvdA : g.val ∣ i.val := hg ▸ Nat.gcd_dvd_left _ _
    have hgdvdB : g.val ∣ i1.val := hg ▸ Nat.gcd_dvd_right _ _
    have hnmg : n_mag.val * g.val = i.val := by rw [hnm]; exact Nat.div_mul_cancel hgdvdA
    have hdmg : d_mag.val * g.val = i1.val := by rw [hdm]; exact Nat.div_mul_cancel hgdvdB
    have hBpos : 0 < i1.val := by
      rw [hi1]; exact Nat.pos_of_ne_zero fun h => hdenv (Int.natAbs_eq_zero.mp h)
    have hdmpos : 0 < d_mag.val := by
      rw [hdm]; exact Nat.div_pos (Nat.le_of_dvd hBpos hgdvdB) hgpos
    have hcop : Nat.Coprime n_mag.val d_mag.val := by
      rw [hnm, hdm, hg]; exact Nat.coprime_div_gcd_div_gcd (hg ▸ hgpos)
    -- the cross-multiplication identity, in ± form (sign supplied per branch)
    by_cases hsign : (decide (num < 0#i128) ^^ decide (den < 0#i128)) = true
    · -- opposite signs: numerator is `-(n_mag)`, denominator `d_mag`
      rw [if_pos hsign]
      -- opposite-sign cross-multiplication identity: -|num|·den = num·|den|
      have hos : -(num.val.natAbs : Int) * den.val = num.val * (den.val.natAbs : Int) := by
        rw [Int.natCast_natAbs, Int.natCast_natAbs]
        by_cases hn : 0 ≤ num.val <;> by_cases hd : 0 ≤ den.val
        · exfalso
          have h1 : ¬ num < 0#i128 := by scalar_tac
          have h2 : ¬ den < 0#i128 := by scalar_tac
          simp_all [Bool.xor]
        · rw [abs_of_nonneg hn, abs_of_neg (not_le.mp hd)]; ring
        · rw [abs_of_neg (not_le.mp hn), abs_of_nonneg hd]; ring
        · exfalso
          have h1 : num < 0#i128 := by scalar_tac
          have h2 : den < 0#i128 := by scalar_tac
          simp_all [Bool.xor]
      step as ⟨o, ho⟩
      by_cases hnfit : (n_mag.val : Int) ≤ i128FitBound
      · rw [if_pos hnfit] at ho
        obtain ⟨val, hov, hvalv⟩ := ho; rw [hov]
        step as ⟨cf, hcf⟩; rw [hcf]
        step as ⟨r, hr⟩
        by_cases hdfit : (d_mag.val : Int) < i128FitBound
        · -- both representable: returns `some { num := -(n_mag), den := d_mag }`
          rw [if_pos hdfit] at hr
          obtain ⟨val1, hrv, hval1v⟩ := hr; rw [hrv]
          step as ⟨o1, ho1⟩; rw [ho1]
          step as ⟨cf1, hcf1⟩; rw [hcf1]
          refine (WP.spec_ok _).mpr ?_
          intro sr hsr; obtain rfl := Option.some.inj hsr
          refine ⟨?_, ?_, ?_⟩
          · rw [hval1v]; exact_mod_cast hdmpos
          · have hna : val.val.natAbs = n_mag.val := by rw [hvalv]; simp
            have hnb : val1.val.natAbs = d_mag.val := by rw [hval1v]; simp
            rw [hna, hnb]; exact hcop
          · rw [hvalv, hval1v]
            apply mul_right_cancel₀ (show (g.val : Int) ≠ 0 from by exact_mod_cast hgpos.ne')
            have e1 : (n_mag.val : Int) * g.val = (num.val.natAbs : Int) := by
              rw [← Nat.cast_mul, hnmg, hi]
            have e2 : (d_mag.val : Int) * g.val = (den.val.natAbs : Int) := by
              rw [← Nat.cast_mul, hdmg, hi1]
            calc -(n_mag.val : Int) * den.val * g.val
                = -((n_mag.val : Int) * g.val) * den.val := by ring
              _ = -(num.val.natAbs : Int) * den.val := by rw [e1]
              _ = num.val * (den.val.natAbs : Int) := hos
              _ = num.val * ((d_mag.val : Int) * g.val) := by rw [e2]
              _ = num.val * d_mag.val * g.val := by ring
        · -- d_mag overflows: returns `none` (vacuous)
          rw [if_neg hdfit] at hr; rw [hr]
          step as ⟨o1, ho1⟩; rw [ho1]
          step as ⟨cf1, hcf1⟩; rw [hcf1]
          refine (WP.spec_ok _).mpr ?_
          intro sr hsr; simp at hsr
      · -- n_mag overflows (neg_mag returns none): `none` (vacuous)
        rw [if_neg hnfit] at ho; rw [ho]
        step as ⟨cf, hcf⟩; rw [hcf]
        refine (WP.spec_ok _).mpr ?_
        intro sr hsr; simp at hsr
    · -- same sign: numerator is `n_mag`, denominator `d_mag`
      rw [if_neg hsign]
      -- same-sign cross-multiplication identity: |num|·den = num·|den|
      have hss : (num.val.natAbs : Int) * den.val = num.val * (den.val.natAbs : Int) := by
        rw [Bool.not_eq_true] at hsign
        rw [Int.natCast_natAbs, Int.natCast_natAbs]
        by_cases hn : 0 ≤ num.val <;> by_cases hd : 0 ≤ den.val
        · rw [abs_of_nonneg hn, abs_of_nonneg hd]
        · exfalso
          have h1 : ¬ num < 0#i128 := by scalar_tac
          have h2 : den < 0#i128 := by scalar_tac
          simp_all [Bool.xor]
        · exfalso
          have h1 : num < 0#i128 := by scalar_tac
          have h2 : ¬ den < 0#i128 := by scalar_tac
          simp_all [Bool.xor]
        · rw [abs_of_neg (not_le.mp hn), abs_of_neg (not_le.mp hd)]; ring
      step as ⟨r, hr⟩
      by_cases hnfit : (n_mag.val : Int) < i128FitBound
      · rw [if_pos hnfit] at hr
        obtain ⟨val, hrv, hvalv⟩ := hr; rw [hrv]
        step as ⟨o, ho⟩; rw [ho]
        step as ⟨cf, hcf⟩; rw [hcf]
        step as ⟨r1, hr1⟩
        by_cases hdfit : (d_mag.val : Int) < i128FitBound
        · -- both fit: returns `some { num := val, den := val1 }`
          rw [if_pos hdfit] at hr1
          obtain ⟨val1, hr1v, hval1v⟩ := hr1; rw [hr1v]
          step as ⟨o1, ho1⟩; rw [ho1]
          step as ⟨cf1, hcf1⟩; rw [hcf1]
          refine (WP.spec_ok _).mpr ?_
          intro sr hsr; obtain rfl := Option.some.inj hsr
          refine ⟨?_, ?_, ?_⟩
          · -- 0 < den
            rw [hval1v]; exact_mod_cast hdmpos
          · -- coprime
            have hna : val.val.natAbs = n_mag.val := by rw [hvalv]; simp
            have hnb : val1.val.natAbs = d_mag.val := by rw [hval1v]; simp
            rw [hna, hnb]; exact hcop
          · -- cross-multiplication: val · den = num · val1
            rw [hvalv, hval1v]
            apply mul_right_cancel₀ (show (g.val : Int) ≠ 0 from by exact_mod_cast hgpos.ne')
            have e1 : (n_mag.val : Int) * g.val = (num.val.natAbs : Int) := by
              rw [← Nat.cast_mul, hnmg, hi]
            have e2 : (d_mag.val : Int) * g.val = (den.val.natAbs : Int) := by
              rw [← Nat.cast_mul, hdmg, hi1]
            calc (n_mag.val : Int) * den.val * g.val
                = (n_mag.val * g.val : Int) * den.val := by ring
              _ = (num.val.natAbs : Int) * den.val := by rw [e1]
              _ = num.val * (den.val.natAbs : Int) := hss
              _ = num.val * ((d_mag.val : Int) * g.val) := by rw [e2]
              _ = num.val * d_mag.val * g.val := by ring
        · -- d_mag overflows: returns `none` (vacuous)
          rw [if_neg hdfit] at hr1; rw [hr1]
          step as ⟨o1, ho1⟩; rw [ho1]
          step as ⟨cf1, hcf1⟩; rw [hcf1]
          refine (WP.spec_ok _).mpr ?_
          intro sr hsr; simp at hsr
      · -- n_mag overflows: returns `none` (vacuous)
        rw [if_neg hnfit] at hr; rw [hr]
        step as ⟨o, ho⟩; rw [ho]
        step as ⟨cf, hcf⟩; rw [hcf]
        refine (WP.spec_ok _).mpr ?_
        intro sr hsr; simp at hsr

end CertifyCheck
