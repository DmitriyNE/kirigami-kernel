/-
  **The reference bignum, proven against ℕ.** (algebra-rehaul R.4b — `docs/refbackend-lift.md`.)

  `lattice::refbackend::RefBackend` is an independent `Vec<u64>`-limb `Backend`, lifted here through
  Aeneas (`Lattice.Funs`). This file proves its `RefNat` limb operations compute the mathematically
  correct answer over a limb→ℕ denotation `den` (little-endian base `2^64`), so the dashu differential
  (`rat::differential`) becomes a proof-backed oracle rather than a cross-check against a trusted model.

  R.4b.1 (this file): the denotation + its core lemmas, and the first two ops —
  `is_zero` (`= true ↔ den = 0`) and `cmp` (`= compare (den ·) (den ·)`). Later phases prove
  `add`/`sub`/`mul`/`divrem`/`gcd` and lift `RefInt`/`RefRat` to `ℤ`/`ℚ`.

  All `RefNat` values in play are **normalized** (no trailing zero limbs) — the representation
  invariant every constructor/op re-establishes via `normalize` (proved: `normalize_normalized`). It
  is carried on the op refinements as an explicit `Normalized` hypothesis on the inputs.
-/
import Mathlib
import Lattice.Funs

namespace CertifyCheck.RefBackend

open Aeneas Aeneas.Std Result
open lattice.refbackend

/-! ### The denotation `den : List U64 → ℕ` (little-endian base `2^64`) -/

/-- `den [x₀, x₁, …] = x₀ + 2⁶⁴·x₁ + 2¹²⁸·x₂ + ⋯` — the natural number a limb list denotes. -/
def den : List Std.U64 → ℕ
  | [] => 0
  | x :: xs => x.val + 2 ^ 64 * den xs

@[simp] theorem den_nil : den [] = 0 := rfl
theorem den_cons (x : Std.U64) (xs) : den (x :: xs) = x.val + 2 ^ 64 * den xs := rfl
@[simp] theorem den_singleton (x : Std.U64) : den [x] = x.val := by simp [den]

/-- Splitting off a high block: `den (a ++ b) = den a + 2^(64·|a|)·den b`. -/
theorem den_append (a b : List Std.U64) :
    den (a ++ b) = den a + 2 ^ (64 * a.length) * den b := by
  induction a with
  | nil => simp [den]
  | cons x xs ih =>
    have hpow : 2 ^ (64 * (xs.length + 1)) = 2 ^ 64 * 2 ^ (64 * xs.length) := by
      rw [Nat.mul_succ, pow_add]; ring
    simp only [List.cons_append, den_cons, ih, List.length_cons]
    rw [hpow]; ring

/-- A trailing zero limb is redundant — the key to `normalize` preserving the denotation. -/
@[simp] theorem den_snoc_zero (l : List Std.U64) : den (l ++ [0#u64]) = den l := by
  rw [den_append]; simp [den]

/-- Every limb is `< 2⁶⁴`, so an `n`-limb value is `< 2^(64n)`. -/
theorem den_lt (l : List Std.U64) : den l < 2 ^ (64 * l.length) := by
  induction l with
  | nil => simp [den]
  | cons x xs ih =>
    have hx : x.val < 2 ^ 64 := by scalar_tac
    have hpow : 2 ^ (64 * (xs.length + 1)) = 2 ^ 64 * 2 ^ (64 * xs.length) := by
      rw [Nat.mul_succ, pow_add]; ring
    simp only [den_cons, List.length_cons]
    rw [hpow]
    have step1 : x.val + 2 ^ 64 * den xs < 2 ^ 64 * (den xs + 1) := by
      rw [Nat.mul_add, Nat.mul_one]; omega
    have step2 : 2 ^ 64 * (den xs + 1) ≤ 2 ^ 64 * 2 ^ (64 * xs.length) :=
      Nat.mul_le_mul_left _ (by omega)
    omega

/-- An `i`-limb prefix denotes `< 2^(64i)`. -/
private theorem den_take_lt (l : List Std.U64) (k : ℕ) (hk : k ≤ l.length) :
    den (l.take k) < 2 ^ (64 * k) := by
  have h := den_lt (l.take k)
  rwa [List.length_take, Nat.min_eq_left hk] at h

/-- Extending a prefix by its next limb `l[k]` (the new most-significant limb). -/
private theorem den_take_succ (l : List Std.U64) (k : ℕ) (hk : k < l.length) :
    den (l.take (k + 1)) = den (l.take k) + 2 ^ (64 * k) * (l[k]).val := by
  rw [← List.take_concat_get' l k hk, den_append]
  have hlen : (l.take k).length = k := by rw [List.length_take]; omega
  rw [hlen, den_singleton]

/-! ### Normalization — the "no trailing zero limbs" representation invariant -/

/-- A limb list is **normalized** when its highest limb (the last element) is nonzero — equivalently,
    no redundant leading zeros in the base-`2^64` numeral. `normalize` re-establishes this. -/
def Normalized (l : List Std.U64) : Prop :=
  ∀ (h : l ≠ []), (l.getLast h).val ≠ 0

/-- A nonempty normalized list denotes at least `2^(64·(n−1))` — its top limb contributes that much,
    so **length is monotone in value**. -/
theorem den_lower (l : List Std.U64) (h : l ≠ []) (hn : Normalized l) :
    2 ^ (64 * (l.length - 1)) ≤ den l := by
  have hlast : (l.getLast h).val ≠ 0 := hn h
  have hlen : l.dropLast.length = l.length - 1 := List.length_dropLast
  have key : den l = den l.dropLast + 2 ^ (64 * (l.length - 1)) * (l.getLast h).val := by
    conv_lhs => rw [← List.dropLast_append_getLast h]
    rw [den_append, hlen, den_singleton]
  rw [key]
  have h1 : (1 : ℕ) ≤ (l.getLast h).val := Nat.one_le_iff_ne_zero.mpr hlast
  calc 2 ^ (64 * (l.length - 1))
      = 2 ^ (64 * (l.length - 1)) * 1 := by ring
    _ ≤ 2 ^ (64 * (l.length - 1)) * (l.getLast h).val := Nat.mul_le_mul_left _ h1
    _ ≤ den l.dropLast + 2 ^ (64 * (l.length - 1)) * (l.getLast h).val := Nat.le_add_left _ _

/-- For normalized lists, `den` vanishes only on the empty list. -/
theorem den_eq_zero_iff (l : List Std.U64) (hn : Normalized l) : den l = 0 ↔ l = [] := by
  constructor
  · intro h0
    by_contra hne
    have hle := den_lower l hne hn
    have hpos : 0 < 2 ^ (64 * (l.length - 1)) := pow_pos (by norm_num) _
    omega
  · rintro rfl; rfl

/-! ### `is_zero` — `Vec::is_empty`, correct as `den = 0` for normalized limbs -/

/-- The lifted `RefNat::is_zero` returns exactly `den = 0` (for normalized limbs). -/
theorem is_zero_eq (self : RefNat) (hn : Normalized self.limbs.val) :
    RefNat.is_zero self = ok (decide (den self.limbs.val = 0)) := by
  unfold RefNat.is_zero alloc.vec.Vec.is_empty
  congr 1
  have hb : self.limbs.val.isEmpty = decide (self.limbs.val = []) := by
    cases self.limbs.val <;> rfl
  rw [hb]
  exact (Bool.decide_congr (den_eq_zero_iff self.limbs.val hn)).symm

/-! ### `cmp` — length-then-MSB-lex equals ℕ order for normalized limbs

`cmp` first compares limb counts, then (if equal) scans MSB→LSB for the first differing limb. The
scan (`cmp_loop`) is proved via `loop.spec_decr_nat` with the *answer-preserving* invariant
"comparing the low `j` limbs gives the same `Ordering` as comparing all `i₀` limbs" (mirroring
`ClipSigma.corner_range_loop_spec`). -/

/-- MSB-dominance: with both low parts `< 2^k`, a differing top limb decides the comparison. -/
private theorem compare_split_ne (p p' s t k : ℕ) (hp : p < 2 ^ k) (hp' : p' < 2 ^ k) (hst : s ≠ t) :
    compare (p + 2 ^ k * s) (p' + 2 ^ k * t) = compare s t := by
  rcases Nat.lt_or_ge s t with h | h
  · have hlt : p + 2 ^ k * s < p' + 2 ^ k * t := by
      have : 2 ^ k * s + 2 ^ k ≤ 2 ^ k * t := by
        rw [← Nat.mul_succ]; exact Nat.mul_le_mul_left _ h
      omega
    rw [compare_lt_iff_lt.mpr hlt, compare_lt_iff_lt.mpr h]
  · have hne : t < s := lt_of_le_of_ne h (Ne.symm hst)
    have hgt : p' + 2 ^ k * t < p + 2 ^ k * s := by
      have : 2 ^ k * t + 2 ^ k ≤ 2 ^ k * s := by
        rw [← Nat.mul_succ]; exact Nat.mul_le_mul_left _ hne
      omega
    rw [compare_gt_iff_gt.mpr hgt, compare_gt_iff_gt.mpr hne]

/-- Equal top limb: the comparison reduces to the low parts. -/
private theorem compare_split_eq (p p' c k : ℕ) :
    compare (p + 2 ^ k * c) (p' + 2 ^ k * c) = compare p p' := by
  rcases lt_trichotomy p p' with h | h | h
  · rw [compare_lt_iff_lt.mpr h, compare_lt_iff_lt.mpr (by omega)]
  · subst h; rw [Std.ReflOrd.compare_self, Std.ReflOrd.compare_self]
  · rw [compare_gt_iff_gt.mpr h, compare_gt_iff_gt.mpr (by omega)]

/-- The MSB→LSB scan computes `compare` of the low `i₀` limbs. -/
private theorem cmp_loop_spec (v v1 : alloc.vec.Vec Std.U64) (i0 : Std.Usize)
    (hv : i0.val ≤ v.val.length) (hv1 : i0.val ≤ v1.val.length) :
    RefNat.cmp_loop v v1 i0
      ⦃ r => r = compare (den (v.val.take i0.val)) (den (v1.val.take i0.val)) ⦄ := by
  unfold RefNat.cmp_loop
  apply loop.spec_decr_nat
    (measure := fun j => j.val)
    (inv := fun j => j.val ≤ v.val.length ∧ j.val ≤ v1.val.length ∧
      compare (den (v.val.take j.val)) (den (v1.val.take j.val))
        = compare (den (v.val.take i0.val)) (den (v1.val.take i0.val)))
  · rintro j ⟨hjv, hjv1, hinv⟩
    show RefNat.cmp_loop.body v v1 j ⦃ _ ⦄
    unfold RefNat.cmp_loop.body
    by_cases hj : j > 0#usize
    · rw [if_pos hj]
      step; step; step
      -- `i2 = v[i1]`, `i3 = v1[i1]` with `i1 = j-1` the top limb of the length-`j` prefix.
      have hkv : i1.val < v.val.length := by scalar_tac
      have hkv1 : i1.val < v1.val.length := by scalar_tac
      have hjeq : j.val = i1.val + 1 := by scalar_tac
      have hdec_v : den (v.val.take j.val)
          = den (v.val.take i1.val) + 2 ^ (64 * i1.val) * i2.val := by
        rw [i2_post, hjeq]; exact den_take_succ v.val i1.val hkv
      have hdec_v1 : den (v1.val.take j.val)
          = den (v1.val.take i1.val) + 2 ^ (64 * i1.val) * i3.val := by
        rw [i3_post, hjeq]; exact den_take_succ v1.val i1.val hkv1
      have hlo_v := den_take_lt v.val i1.val (le_of_lt hkv)
      have hlo_v1 := den_take_lt v1.val i1.val (le_of_lt hkv1)
      rw [hdec_v, hdec_v1] at hinv
      by_cases hbeq : i2 = i3
      · -- equal top limb: recurse at `i1`, the answer-preserving invariant carried down.
        subst hbeq
        rw [compare_split_eq] at hinv
        simp only [bne_self_eq_false, Bool.false_eq_true, if_false, WP.spec_ok]
        exact ⟨le_of_lt hkv, le_of_lt hkv1, hinv, by omega⟩
      · -- differing top limb decides the comparison (MSB dominance).
        have hval_ne : i2.val ≠ i3.val := fun h => hbeq (UScalar.val_eq_imp i2 i3 h)
        rw [compare_split_ne _ _ _ _ _ hlo_v hlo_v1 hval_ne] at hinv
        rw [if_pos (by simp only [bne_iff_ne]; exact hbeq)]
        simp only [core.cmp.impls.OrdU64.cmp, lift, bind_tc_ok, WP.spec_ok]
        exact hinv
    · -- j = 0: the scan bottoms out at `eq`; the invariant says that IS the answer.
      rw [if_neg hj]
      have hj0 : j.val = 0 := by scalar_tac
      simp only [WP.spec_ok]
      rw [← hinv, hj0]
      simp [Std.ReflOrd.compare_self]
  · exact ⟨hv, hv1, rfl⟩

/-- Length monotonicity: for normalized lists of differing length, comparing lengths *is* comparing
    values (the longer numeral, having a nonzero top limb, is the larger). -/
private theorem len_compare (a b : List Std.U64) (ha : Normalized a) (hb : Normalized b)
    (hne : a.length ≠ b.length) : compare a.length b.length = compare (den a) (den b) := by
  have key : ∀ x y : List Std.U64, Normalized y → x.length < y.length → den x < den y := by
    intro x y hy hlt
    have hxne : y ≠ [] := by rintro rfl; simp at hlt
    have h1 : den x < 2 ^ (64 * x.length) := den_lt x
    have h2 : (2 : ℕ) ^ (64 * x.length) ≤ 2 ^ (64 * (y.length - 1)) :=
      Nat.pow_le_pow_right (by norm_num) (by omega)
    have h3 : 2 ^ (64 * (y.length - 1)) ≤ den y := den_lower y hxne hy
    omega
  rcases Nat.lt_or_gt_of_ne hne with h | h
  · rw [compare_lt_iff_lt.mpr h, compare_lt_iff_lt.mpr (key a b hb h)]
  · rw [compare_gt_iff_gt.mpr h, compare_gt_iff_gt.mpr (key b a ha h)]

/-- **`cmp` refinement.** The lifted `RefNat::cmp` computes `compare (den ·) (den ·)` on the limb
    denotations (for normalized limbs) — the base-`2^64` comparison is ℕ order. -/
theorem cmp_eq (a b : RefNat) (ha : Normalized a.limbs.val) (hb : Normalized b.limbs.val) :
    RefNat.cmp a b = ok (compare (den a.limbs.val) (den b.limbs.val)) := by
  unfold RefNat.cmp
  have hlenA : (a.limbs.len).val = a.limbs.val.length := alloc.vec.Vec.len_val a.limbs
  have hlenB : (b.limbs.len).val = b.limbs.val.length := alloc.vec.Vec.len_val b.limbs
  by_cases hlen : a.limbs.val.length = b.limbs.val.length
  · -- equal length: the MSB scan over the full lists
    have hlens : a.limbs.len = b.limbs.len := UScalar.val_eq_imp _ _ (by rw [hlenA, hlenB, hlen])
    have hcond : (a.limbs.len != b.limbs.len) = false := by rw [hlens]; exact bne_self_eq_false _
    simp only [hcond, Bool.false_eq_true, if_false]
    have hspec := cmp_loop_spec a.limbs b.limbs a.limbs.len (le_of_eq hlenA) (hlenA.trans hlen).le
    rw [hlenA, List.take_length, List.take_of_length_le (le_of_eq hlen.symm)] at hspec
    cases h : RefNat.cmp_loop a.limbs b.limbs a.limbs.len with
    | ok r => rw [h] at hspec; simp only [WP.spec_ok] at hspec; rw [hspec]
    | fail e => rw [h] at hspec; exact hspec.elim
    | div => rw [h] at hspec; exact hspec.elim
  · -- different length: length order = value order (len-monotone)
    have hcond : (a.limbs.len != b.limbs.len) = true := by
      rw [bne_iff_ne, ne_eq]
      exact fun h => hlen (by rw [← hlenA, ← hlenB, h])
    simp only [hcond, if_true, core.cmp.impls.OrdUsize.cmp]
    rw [hlenA, hlenB, len_compare a.limbs.val b.limbs.val ha hb hlen]

/-! ### `normalize` preserves the denotation (it only drops trailing zero limbs) -/

/-- The lifted `normalize` pops trailing zero limbs, so it leaves the denotation unchanged. -/
private theorem normalize_den (l : alloc.vec.Vec Std.U64) :
    lattice.refbackend.normalize l ⦃ r => den r.val = den l.val ⦄ := by
  unfold lattice.refbackend.normalize normalize_loop
  apply loop.spec_decr_nat
    (measure := fun s => s.val.length)
    (inv := fun s => den s.val = den l.val)
  · rintro s hinv
    show normalize_loop.body s ⦃ _ ⦄
    unfold normalize_loop.body alloc.vec.Vec.is_empty
    simp only [bind_tc_ok]
    by_cases hb : (s.val).isEmpty = true
    · rw [if_pos hb]; simp only [WP.spec_ok]; exact hinv
    · rw [if_neg hb]
      have hne : s.val ≠ [] := by simpa using hb
      have hlen1 : 0 < s.len.val := by
        rw [alloc.vec.Vec.len_val]; exact List.length_pos_of_ne_nil hne
      step
      step
      have hlen0 : 0 < s.val.length := List.length_pos_of_ne_nil hne
      have hidx : i1.val = s.val.length - 1 := by rw [i1_post1, alloc.vec.Vec.len_val]
      have hidx_lt : i1.val < s.val.length := by omega
      by_cases hz : i2 = 0#u64
      · -- the top limb is zero: pop it (den unchanged, length strictly drops).
        rw [if_pos hz]
        unfold alloc.vec.Vec.pop
        simp only [bind_tc_ok]
        have hden : den s.val.dropLast = den s.val := by
          have hsucc := den_take_succ s.val i1.val hidx_lt
          rw [(by omega : i1.val + 1 = s.val.length), List.take_length] at hsucc
          have hi2z : (s.val[i1.val]'hidx_lt).val = 0 := by rw [← i2_post, hz]; rfl
          rw [List.dropLast_eq_take, ← hidx, hsucc, hi2z]; ring
        exact ⟨by rw [hden]; exact hinv, by rw [List.length_dropLast]; omega⟩
      · rw [if_neg hz]; simp only [WP.spec_ok]; exact hinv
  · rfl

/-- `normalize` yields a normalized list — it strips trailing zero limbs, stopping at a nonzero top. -/
private theorem normalize_normalized (l : alloc.vec.Vec Std.U64) :
    lattice.refbackend.normalize l ⦃ r => Normalized r.val ⦄ := by
  unfold lattice.refbackend.normalize normalize_loop
  apply loop.spec_decr_nat
    (measure := fun s => s.val.length)
    (inv := fun _ => True)
  · rintro s _
    show normalize_loop.body s ⦃ _ ⦄
    unfold normalize_loop.body alloc.vec.Vec.is_empty
    simp only [bind_tc_ok]
    by_cases hb : (s.val).isEmpty = true
    · rw [if_pos hb]; simp only [WP.spec_ok]
      have hemp : s.val = [] := by simpa using hb
      intro h; exact absurd hemp h
    · rw [if_neg hb]
      have hne : s.val ≠ [] := by simpa using hb
      have hlen1 : 0 < s.len.val := by rw [alloc.vec.Vec.len_val]; exact List.length_pos_of_ne_nil hne
      step; step
      have hlen0 : 0 < s.val.length := List.length_pos_of_ne_nil hne
      have hidx : i1.val = s.val.length - 1 := by rw [i1_post1, alloc.vec.Vec.len_val]
      have hidx_lt : i1.val < s.val.length := by omega
      by_cases hz : i2 = 0#u64
      · rw [if_pos hz]; unfold alloc.vec.Vec.pop; simp only [bind_tc_ok]
        show s.val.dropLast.length < s.val.length
        rw [List.length_dropLast]; omega
      · rw [if_neg hz]; simp only [WP.spec_ok]
        intro h
        have hgl : i2 = s.val.getLast h := by rw [i2_post, List.getLast_eq_getElem]; congr 1
        rw [← hgl]
        exact fun hc => hz (Std.UScalar.eq_of_val_eq (by rw [hc]; rfl))
  · trivial

/-! ### `add` — the schoolbook carry loop computes `den a + den b`

The one non-bookkeeping ingredient is the **`u128` split**: writing `s = a + b + carry`, the emitted
limb `s as u64` is `s mod 2⁶⁴` and the new carry `s >>> 64` is `s / 2⁶⁴`, so
`(s as u64) + 2⁶⁴·(s >>> 64) = s`. -/

/-- The truncating `u128 → u64` cast keeps the low 64 bits. -/
private theorem cast_u64_val (s : Std.U128) : (UScalar.cast .U64 s).val = s.val % 2 ^ 64 := by
  simp only [UScalar.cast, UScalar.val, BitVec.toNat_setWidth, UScalarTy.numBits]

/-- The `u128` limb/carry split: `(s as u64) + 2⁶⁴·(s >>> 64) = s`. -/
private theorem u128_split (s : Std.U128) :
    (UScalar.cast .U64 s).val + 2 ^ 64 * (s.val >>> (64 : ℕ)) = s.val := by
  rw [cast_u64_val, Nat.shiftRight_eq_div_pow]
  exact Nat.mod_add_div s.val (2 ^ 64)

/-- The widening `u64 → u128` cast preserves the value. -/
private theorem cast_u128_val (x : Std.U64) : (UScalar.cast .U128 x).val = x.val := by
  simp only [UScalar.cast, UScalar.val, BitVec.toNat_setWidth, UScalarTy.numBits]
  apply Nat.mod_eq_of_lt
  calc x.bv.toNat < 2 ^ 64 := x.bv.isLt
    _ ≤ 2 ^ 128 := Nat.pow_le_pow_right (by norm_num) (by norm_num)

/-- Unified `take`-successor: the new most-significant limb is `l[i]` (or `0` past the end). -/
private theorem den_take_succ_pad (l : List Std.U64) (i : ℕ) :
    den (l.take (i + 1)) = den (l.take i)
      + 2 ^ (64 * i) * (if h : i < l.length then (l[i]'h).val else 0) := by
  by_cases h : i < l.length
  · rw [dif_pos h, den_take_succ l i h]
  · rw [dif_neg h, List.take_of_length_le (by omega), List.take_of_length_le (by omega),
      mul_zero, add_zero]

/-- `add`'s carry loop, at step `i`: the emitted `i` limbs plus the pending carry (always `≤ 1`)
    account for the low-`i` denotations of both inputs (padding missing high limbs with `0`). -/
private theorem add_loop_spec (v v1 : alloc.vec.Vec Std.U64) (n : Std.Usize)
    (out : alloc.vec.Vec Std.U64) (carry : Std.U128) (i : Std.Usize) (hi : i.val ≤ n.val)
    (hlen : out.val.length = i.val) (hcar : carry.val ≤ 1)
    (hinv : den out.val + carry.val * 2 ^ (64 * i.val)
      = den (v.val.take i.val) + den (v1.val.take i.val)) :
    RefNat.add_loop v v1 n out carry i
      ⦃ r => (den r.1.val + r.2.val * 2 ^ (64 * n.val)
          = den (v.val.take n.val) + den (v1.val.take n.val))
        ∧ r.1.val.length = n.val ∧ r.2.val ≤ 1 ⦄ := by
  unfold RefNat.add_loop
  apply loop.spec_decr_nat
    (measure := fun st => n.val - st.2.2.val)
    (inv := fun st => st.2.2.val ≤ n.val ∧ st.1.val.length = st.2.2.val ∧ st.2.1.val ≤ 1 ∧
      den st.1.val + st.2.1.val * 2 ^ (64 * st.2.2.val)
        = den (v.val.take st.2.2.val) + den (v1.val.take st.2.2.val))
  · rintro ⟨out', carry', i'⟩ ⟨hi', hlen', hcar', hinv'⟩
    show RefNat.add_loop.body v v1 n out' carry' i' ⦃ _ ⦄
    unfold RefNat.add_loop.body
    simp only [] at hi' hlen' hcar' hinv'
    by_cases hlt : i' < n
    · rw [if_pos hlt]
      dsimp only []
      have hvlen : (v.len).val = v.val.length := alloc.vec.Vec.len_val v
      have hv1len : (v1.len).val = v1.val.length := alloc.vec.Vec.len_val v1
      -- resolve the two padded limb loads to concrete `u128` values
      set av : Std.U128 := if h : i'.val < v.val.length
        then UScalar.cast .U128 (v.val[i'.val]'h) else 0#u128 with hav_def
      set bv : Std.U128 := if h : i'.val < v1.val.length
        then UScalar.cast .U128 (v1.val[i'.val]'h) else 0#u128 with hbv_def
      have ha : (if i' < v.len then (do
          let i2 ← alloc.vec.Vec.index (core.slice.index.SliceIndexUsizeSlice Std.U64) v i'
          ok (UScalar.cast .U128 i2)) else ok 0#u128) = ok av := by
        rw [alloc.vec.Vec.index_slice_index]
        by_cases h : i'.val < v.val.length
        · rw [if_pos (by scalar_tac), hav_def, dif_pos h,
            show v.index_usize i' = ok (v.val[i'.val]'h) from by
              simp only [alloc.vec.Vec.index_usize, alloc.vec.Vec.getElem?_Nat_eq,
                List.getElem?_eq_getElem h]]
          rfl
        · rw [if_neg (by scalar_tac), hav_def, dif_neg h]
      have hb : (if i' < v1.len then (do
          let i2 ← alloc.vec.Vec.index (core.slice.index.SliceIndexUsizeSlice Std.U64) v1 i'
          ok (UScalar.cast .U128 i2)) else ok 0#u128) = ok bv := by
        rw [alloc.vec.Vec.index_slice_index]
        by_cases h : i'.val < v1.val.length
        · rw [if_pos (by scalar_tac), hbv_def, dif_pos h,
            show v1.index_usize i' = ok (v1.val[i'.val]'h) from by
              simp only [alloc.vec.Vec.index_usize, alloc.vec.Vec.getElem?_Nat_eq,
                List.getElem?_eq_getElem h]]
          rfl
        · rw [if_neg (by scalar_tac), hbv_def, dif_neg h]
      -- bounds on the padded values
      have hav_lt : av.val < 2 ^ 64 := by
        rw [hav_def]; split
        · rw [cast_u128_val]; have := (v.val[i'.val]'(by assumption)).hBounds
          simp only [UScalar.val] at this ⊢; omega
        · simp
      have hbv_lt : bv.val < 2 ^ 64 := by
        rw [hbv_def]; split
        · rw [cast_u128_val]; have := (v1.val[i'.val]'(by assumption)).hBounds
          simp only [UScalar.val] at this ⊢; omega
        · simp
      rw [ha, hb]
      -- symbolically execute the u128 arithmetic
      step; step; step; step; step; step
      -- padded-value forms + the u128 split, for the den bookkeeping
      have hav_eq : av.val = (if h : i'.val < v.val.length then (v.val[i'.val]'h).val else 0) := by
        rw [hav_def]; split
        · exact cast_u128_val _
        · rfl
      have hbv_eq : bv.val = (if h : i'.val < v1.val.length then (v1.val[i'.val]'h).val else 0) := by
        rw [hbv_def]; split
        · exact cast_u128_val _
        · rfl
      have hsval : s.val = av.val + bv.val + carry'.val := by rw [s_post, a_post]
      have hsplit : i4.val + 2 ^ 64 * carry1.val = s.val := by
        rw [i4_post, carry1_post1]; exact u128_split s
      have hs65 : s.val < 2 ^ 65 := by
        have hp : (2 : ℕ) ^ 64 + 2 ^ 64 = 2 ^ 65 := by ring
        rw [hsval]; omega
      refine ⟨by scalar_tac, ?_, ?_, ?_, by scalar_tac⟩
      · rw [out1_post, List.length_append, hlen', i5_post]; simp
      · rw [carry1_post1, Nat.shiftRight_eq_div_pow]
        have hlt2 : s.val / 2 ^ 64 < 2 := (Nat.div_lt_iff_lt_mul (by positivity)).mpr (by
          have hp : (2 : ℕ) * 2 ^ 64 = 2 ^ 65 := by ring
          omega)
        omega
      · -- the denotation carry-invariant at i'+1
        have hpow : (2 : ℕ) ^ (64 * i5.val) = 2 ^ (64 * i'.val) * 2 ^ 64 := by
          rw [i5_post, Nat.mul_add, Nat.mul_one, pow_add]
        have hcarry_grp : 2 ^ (64 * i'.val) * i4.val
            + carry1.val * (2 ^ (64 * i'.val) * 2 ^ 64)
            = 2 ^ (64 * i'.val) * (av.val + bv.val + carry'.val) := by
          have hr : 2 ^ (64 * i'.val) * i4.val + carry1.val * (2 ^ (64 * i'.val) * 2 ^ 64)
              = 2 ^ (64 * i'.val) * (i4.val + 2 ^ 64 * carry1.val) := by ring
          rw [hr, hsplit, hsval]
        rw [out1_post, den_append, den_singleton, hlen', hpow, i5_post,
          den_take_succ_pad v.val i'.val, den_take_succ_pad v1.val i'.val, ← hav_eq, ← hbv_eq,
          add_assoc, hcarry_grp, mul_add, mul_add]
        have hc := hinv'
        rw [mul_comm carry'.val] at hc
        omega
    · rw [if_neg hlt]
      simp only [WP.spec_ok]
      have hin : i'.val = n.val := by scalar_tac
      rw [hin] at hinv' hlen'
      exact ⟨hinv', hlen', hcar'⟩
  · exact ⟨hi, hlen, hcar, hinv⟩

/-- The `add` loop result, plus the optional final carry limb and `normalize`, denotes `den a + den b`
    — the shared tail of `add` for any `n` = `max` of the two limb counts. -/
private theorem add_tail (a b : RefNat) (n : Std.Usize)
    (hnmax : n.val = max a.limbs.val.length b.limbs.val.length) (hcap : n.val + 1 ≤ Std.Usize.max) :
    (do
      let i2 ← n + 1#usize
      let out := alloc.vec.Vec.with_capacity Std.U64 i2
      let r ← RefNat.add_loop a.limbs b.limbs n out 0#u128 0#usize
      let out2 ← if r.2 != 0#u128 then (do
          let i3 ← lift (UScalar.cast .U64 r.2); alloc.vec.Vec.push r.1 i3) else ok r.1
      let out3 ← lattice.refbackend.normalize out2
      ok ({ limbs := out3 } : RefNat))
    ⦃ r => den r.limbs.val = den a.limbs.val + den b.limbs.val ∧ Normalized r.limbs.val ⦄ := by
  have hna : a.limbs.val.length ≤ n.val := by rw [hnmax]; omega
  have hnb : b.limbs.val.length ≤ n.val := by rw [hnmax]; omega
  step
  simp only [alloc.vec.Vec.with_capacity]
  have hloop := add_loop_spec a.limbs b.limbs n (alloc.vec.Vec.new Std.U64) 0#u128 0#usize
    (by simp) (by simp) (by simp) (by simp [den])
  cases hcase : RefNat.add_loop a.limbs b.limbs n (alloc.vec.Vec.new Std.U64) 0#u128 0#usize with
  | ok r =>
    obtain ⟨out1, carry⟩ := r
    rw [hcase] at hloop; simp only [WP.spec_ok] at hloop
    obtain ⟨hden, hlen1, hcar1⟩ := hloop
    rw [List.take_of_length_le hna, List.take_of_length_le hnb] at hden
    have hout2 : ∀ out2 : alloc.vec.Vec Std.U64, den out2.val = den a.limbs.val + den b.limbs.val →
        (do let out3 ← lattice.refbackend.normalize out2; ok ({ limbs := out3 } : RefNat))
          ⦃ r => den r.limbs.val = den a.limbs.val + den b.limbs.val ∧ Normalized r.limbs.val ⦄ := by
      intro out2 hout2den
      have hnorm := normalize_den out2
      have hnn := normalize_normalized out2
      cases hnc : lattice.refbackend.normalize out2 with
      | ok o =>
        rw [hnc] at hnorm hnn; simp only [WP.spec_ok] at hnorm hnn
        simp only [bind_tc_ok, WP.spec_ok]
        exact ⟨by show den o.val = _; rw [hnorm, hout2den], hnn⟩
      | fail e => rw [hnc] at hnorm; exact hnorm.elim
      | div => rw [hnc] at hnorm; exact hnorm.elim
    simp only [bind_tc_ok]
    by_cases hcz : carry = 0#u128
    · subst hcz
      simp only [bne_self_eq_false, Bool.false_eq_true, if_false, bind_tc_ok]
      apply hout2
      simpa using hden
    · rw [if_pos (by simp only [ne_eq, bne_iff_ne]; exact hcz)]
      step
      step
      rename_i i3 hi3
      apply hout2
      rw [out2_post, den_append, den_singleton, hlen1, hi3, cast_u64_val,
        Nat.mod_eq_of_lt (lt_of_le_of_lt hcar1 (by norm_num))]
      rw [mul_comm] at hden; omega
  | fail e => rw [hcase] at hloop; exact hloop.elim
  | div => rw [hcase] at hloop; exact hloop.elim

/-- **`add` refinement.** The lifted `RefNat::add` computes `den a + den b` on the limb denotations
    (for inputs whose limb count leaves room for the carry — always the case in practice). -/
theorem add_eq (a b : RefNat)
    (hcap : max a.limbs.val.length b.limbs.val.length + 1 ≤ Std.Usize.max) :
    RefNat.add a b
      ⦃ r => den r.limbs.val = den a.limbs.val + den b.limbs.val ∧ Normalized r.limbs.val ⦄ := by
  unfold RefNat.add
  dsimp only []
  have hlA : (a.limbs.len).val = a.limbs.val.length := alloc.vec.Vec.len_val a.limbs
  have hlB : (b.limbs.len).val = b.limbs.val.length := alloc.vec.Vec.len_val b.limbs
  by_cases hge : a.limbs.len ≥ b.limbs.len
  · rw [if_pos hge]
    simp only [bind_tc_ok]
    have hge' : b.limbs.val.length ≤ a.limbs.val.length := by scalar_tac
    apply add_tail a b a.limbs.len
    · rw [hlA]; omega
    · rw [hlA]; omega
  · rw [if_neg hge]
    simp only [bind_tc_ok]
    have hge' : a.limbs.val.length ≤ b.limbs.val.length := by scalar_tac
    apply add_tail a b b.limbs.len
    · rw [hlB]; omega
    · rw [hlB]; omega

/-- Widening `u64 → i128` (zero-extend) preserves the value. -/
private theorem uhcast_i128_val (x : Std.U64) : (UScalar.hcast .I128 x).val = (x.val : ℤ) := by
  have h := UScalar.hcast_inBounds_spec .I128 x (by scalar_tac)
  simpa [lift, WP.spec_ok] using h

/-- Truncating `i128 → u64`: for a value in `[0, 2⁶⁴)` the low 64 bits are exactly that value. -/
private theorem ihcast_u64_val (d : Std.I128) (h0 : 0 ≤ d.val) (h64 : d.val < 2 ^ 64) :
    ((IScalar.hcast .U64 d).val : ℤ) = d.val := by
  have h := IScalar.hcast_inBounds_spec .U64 d ⟨h0, by scalar_tac⟩
  simpa [lift, WP.spec_ok] using h

/-! ### `sub` — the schoolbook borrow loop computes `den self − den o`

Dual of `add`, but the per-limb arithmetic is signed `i128` (`d = a − b − borrow`, with a `d < 0`
branch), so the loop invariant lives over ℤ. The final borrow is `0` exactly when `den o ≤ den self`
(the Rust contract), which forces the truncated result to be `den self − den o`. -/

set_option maxHeartbeats 800000 in
/-- `sub`'s borrow loop, at step `i` (over ℤ): the emitted `i` limbs plus the pending borrow account
    for the low-`i` denotation difference. The final borrow (existential) is `0`/`1`. -/
private theorem sub_loop_spec (self o : RefNat) (out : alloc.vec.Vec Std.U64)
    (borrow : Std.I128) (i : Std.Usize) (hlo : o.limbs.val.length ≤ self.limbs.val.length)
    (hi : i.val ≤ self.limbs.val.length) (hlen : out.val.length = i.val)
    (hbor : borrow.val = 0 ∨ borrow.val = 1)
    (hinv : (den (self.limbs.val.take i.val) : ℤ) + borrow.val * 2 ^ (64 * i.val)
      = (den out.val : ℤ) + (den (o.limbs.val.take i.val) : ℤ)) :
    RefNat.sub_loop self.limbs o out borrow i
      ⦃ r => (∃ bf : ℤ, (bf = 0 ∨ bf = 1) ∧
          (den self.limbs.val : ℤ) + bf * 2 ^ (64 * self.limbs.val.length)
            = (den r.val : ℤ) + (den o.limbs.val : ℤ)) ∧ r.val.length = self.limbs.val.length ⦄ := by
  unfold RefNat.sub_loop
  apply loop.spec_decr_nat
    (measure := fun st => self.limbs.val.length - st.2.2.val)
    (inv := fun st => st.2.2.val ≤ self.limbs.val.length ∧ st.1.val.length = st.2.2.val ∧
      (st.2.1.val = 0 ∨ st.2.1.val = 1) ∧
      (den (self.limbs.val.take st.2.2.val) : ℤ) + st.2.1.val * 2 ^ (64 * st.2.2.val)
        = (den st.1.val : ℤ) + (den (o.limbs.val.take st.2.2.val) : ℤ))
  · rintro ⟨out', borrow', i'⟩ ⟨hi', hlen', hbor', hinv'⟩
    show RefNat.sub_loop.body self.limbs o out' borrow' i' ⦃ _ ⦄
    unfold RefNat.sub_loop.body
    simp only [] at hi' hlen' hbor' hinv'
    dsimp only []
    by_cases hlt : i' < self.limbs.len
    · rw [if_pos hlt]
      step; step
      have hiv : i'.val < self.limbs.val.length := by scalar_tac
      have ha_val : a.val = ((self.limbs.val[i'.val]'hiv).val : ℤ) := by
        rw [a_post, uhcast_i128_val, i2_post]
      have ha_lt : a.val < 2 ^ 64 := by rw [ha_val]; exact_mod_cast (self.limbs.val[i'.val]'hiv).bv.isLt
      have ha_ge : (0 : ℤ) ≤ a.val := by rw [ha_val]; positivity
      set bv : Std.I128 := if h : i'.val < o.limbs.val.length
        then UScalar.hcast .I128 (o.limbs.val[i'.val]'h) else 0#i128 with hbv_def
      have hb : (if i' < o.limbs.len then (do
          let i4 ← o.limbs.index_usize i'
          ok (UScalar.hcast .I128 i4)) else ok 0#i128) = ok bv := by
        by_cases h : i'.val < o.limbs.val.length
        · rw [if_pos (by scalar_tac), hbv_def, dif_pos h,
            show o.limbs.index_usize i' = ok (o.limbs.val[i'.val]'h) from by
              simp only [alloc.vec.Vec.index_usize, alloc.vec.Vec.getElem?_Nat_eq,
                List.getElem?_eq_getElem h]]
          rfl
        · rw [if_neg (by scalar_tac), hbv_def, dif_neg h]
      have hbv_val : bv.val = (if h : i'.val < o.limbs.val.length
          then ((o.limbs.val[i'.val]'h).val : ℤ) else 0) := by
        rw [hbv_def]; split
        · exact uhcast_i128_val _
        · rfl
      have hbv_lt : bv.val < 2 ^ 64 := by
        rw [hbv_val]; split
        · exact_mod_cast (o.limbs.val[i'.val]'(by assumption)).bv.isLt
        · norm_num
      have hbv_ge : (0 : ℤ) ≤ bv.val := by rw [hbv_val]; split <;> positivity
      have hbor'0 : 0 ≤ borrow'.val := by rcases hbor' with h | h <;> omega
      have hbor'1 : borrow'.val ≤ 1 := by rcases hbor' with h | h <;> omega
      have hpad_s : (den (self.limbs.val.take (i'.val + 1)) : ℤ)
          = (den (self.limbs.val.take i'.val) : ℤ) + 2 ^ (64 * i'.val) * a.val := by
        rw [den_take_succ_pad, dif_pos hiv, ha_val]; push_cast; ring
      have hpad_o : (den (o.limbs.val.take (i'.val + 1)) : ℤ)
          = (den (o.limbs.val.take i'.val) : ℤ) + 2 ^ (64 * i'.val) * bv.val := by
        rw [den_take_succ_pad, hbv_val]; split <;> push_cast <;> ring
      rw [hb]
      step; step
      -- shared den reconstruction as a plain ℤ identity (an explicit `⦃match⦄` post would compile to
      -- a different match-motive than the loop body's, so it never unifies — inline the `step`s instead).
      have recon : ∀ (bc dd : ℤ) (i5 : Std.U64) (out1 : alloc.vec.Vec Std.U64) (i6 : Std.Usize),
          i6.val = i'.val + 1 → out1.val = out'.val ++ [i5] → (i5.val : ℤ) = dd →
          a.val - bv.val - borrow'.val + bc * 2 ^ 64 = dd →
          (den (self.limbs.val.take i6.val) : ℤ) + bc * 2 ^ (64 * i6.val)
            = (den out1.val : ℤ) + (den (o.limbs.val.take i6.val) : ℤ) := by
        intro bc dd i5 out1 i6 hi6 hout1 hlov hrel
        have hpow : (2 : ℤ) ^ (64 * i6.val) = 2 ^ (64 * i'.val) * 2 ^ 64 := by
          rw [hi6, Nat.mul_add, Nat.mul_one, pow_add]
        rw [hpow, hi6, hpad_s, hpad_o, hout1, den_append, den_singleton, hlen']
        push_cast [hlov]
        linear_combination hinv' + (2 : ℤ) ^ (64 * i'.val) * hrel
      have hd_val : d.val = a.val - bv.val - borrow'.val := by rw [d_post, b_post]
      have hd_lo : -2 ^ 64 ≤ d.val := by rw [hd_val]; omega
      have hd_hi : d.val < 2 ^ 64 := by rw [hd_val]; omega
      by_cases hd : d < 0#i128
      · rw [if_pos hd]
        step; step
        have hd0 : d.val < 0 := by scalar_tac
        have hshift : borrow1.val = 2 ^ 64 := by
          rw [d1, Int.shiftLeft_eq, one_mul]
          refine Int.bmod_eq_of_le_mul_two ?_ ?_ <;> scalar_tac
        have hxlo : 0 ≤ x.val := by rw [x_post, hshift]; omega
        have hxhi : x.val < 2 ^ 64 := by rw [x_post, hshift]; omega
        step; step
        · scalar_tac
        step
        · scalar_tac
        have hlov : (i5.val : ℤ) = x.val := by rw [i5_post]; exact ihcast_u64_val x hxlo hxhi
        refine ⟨by scalar_tac, ?_, Or.inr (by decide), ?_, by scalar_tac⟩
        · rw [out1_post, List.length_append, hlen', i6_post]; simp
        · refine recon _ _ i5 out1 i6 i6_post out1_post hlov ?_
          rw [x_post, hshift, hd_val]; norm_num
      · rw [if_neg hd]
        simp only [bind_tc_ok]
        have hd0 : 0 ≤ d.val := by scalar_tac
        step; step
        · scalar_tac
        step
        · scalar_tac
        have hlov : (r.val : ℤ) = d.val := by rw [r_post]; exact ihcast_u64_val d hd0 hd_hi
        refine ⟨by scalar_tac, ?_, Or.inl (by decide), ?_, by scalar_tac⟩
        · rw [out1_post, List.length_append, hlen', i6_post]; simp
        · refine recon _ _ r out1 i6 i6_post out1_post hlov ?_
          rw [hd_val]; norm_num
    · rw [if_neg hlt]
      simp only [WP.spec_ok]
      have hin : i'.val = self.limbs.val.length := by scalar_tac
      rw [hin] at hinv' hlen'
      rw [List.take_length, List.take_of_length_le hlo] at hinv'
      exact ⟨⟨borrow'.val, hbor', by linarith [hinv']⟩, hlen'⟩
  · exact ⟨hi, hlen, hbor, hinv⟩

/-- **`sub` refinement.** For `den o ≤ den self` (and `len o ≤ len self`), the lifted `RefNat::sub`
    computes `den self − den o` on the limb denotations. -/
theorem sub_eq (self o : RefNat) (hlo : o.limbs.val.length ≤ self.limbs.val.length)
    (hle : den o.limbs.val ≤ den self.limbs.val) :
    RefNat.sub self o
      ⦃ r => den r.limbs.val = den self.limbs.val - den o.limbs.val ∧ Normalized r.limbs.val ⦄ := by
  unfold RefNat.sub
  simp only [alloc.vec.Vec.with_capacity]
  have hloop := sub_loop_spec self o (alloc.vec.Vec.new Std.U64) 0#i128 0#usize hlo
    (by simp) (by simp) (Or.inl rfl) (by simp [den])
  cases hcase : RefNat.sub_loop self.limbs o (alloc.vec.Vec.new Std.U64) 0#i128 0#usize with
  | ok out1 =>
    rw [hcase] at hloop; simp only [WP.spec_ok] at hloop
    obtain ⟨⟨bf, hbf, heq⟩, hlen1⟩ := hloop
    have hout1lt : (den out1.val : ℤ) < 2 ^ (64 * self.limbs.val.length) := by
      have h := den_lt out1.val; rw [hlen1] at h; exact_mod_cast h
    have hleZ : (den o.limbs.val : ℤ) ≤ (den self.limbs.val : ℤ) := by exact_mod_cast hle
    have hbf0 : bf = 0 := by
      rcases hbf with h | h
      · exact h
      · exfalso; rw [h] at heq; omega
    rw [hbf0, zero_mul, add_zero] at heq
    have heqN : den self.limbs.val = den out1.val + den o.limbs.val := by exact_mod_cast heq
    simp only [bind_tc_ok]
    have hnorm := normalize_den out1
    have hnn := normalize_normalized out1
    cases hnc : lattice.refbackend.normalize out1 with
    | ok o2 =>
      rw [hnc] at hnorm hnn; simp only [WP.spec_ok] at hnorm hnn
      simp only [bind_tc_ok, WP.spec_ok]
      exact ⟨by show den o2.val = den self.limbs.val - den o.limbs.val; rw [hnorm]; omega, hnn⟩
    | fail e => rw [hnc] at hnorm; exact hnorm.elim
    | div => rw [hnc] at hnorm; exact hnorm.elim
  | fail e => rw [hcase] at hloop; exact hloop.elim
  | div => rw [hcase] at hloop; exact hloop.elim

/-! ### `mul` — the nested schoolbook multiply computes `den self · den o`

Unlike `add`/`sub` (which build the result by appending), `mul` writes `out` **in place** at
`out[i+j]` / `out[k]` via `index_mut`, so the workhorse is `den` under `List.set`. Three loops:
the inner `j`-loop accumulates one row `self[i]·o`, the `k`-loop propagates its final carry, and the
outer `i`-loop sums the rows. The magnitude bound `den(partial) < 2^(64·(i+1+m))` keeps the carry
loop in bounds (`k < len out`). -/

/-- In-place limb update: `den (l.set p x)` adjusts `den l` by `(x − l[p])·2^(64p)` at position `p`. -/
private theorem den_set (l : List Std.U64) (p : ℕ) (x : Std.U64) (h : p < l.length) :
    (den (l.set p x) : ℤ) = (den l : ℤ) + ((x.val : ℤ) - ((l[p]'h).val : ℤ)) * 2 ^ (64 * p) := by
  induction l generalizing p with
  | nil => exact absurd h (by simp)
  | cons a tl ih =>
    cases p with
    | zero =>
      simp only [List.set_cons_zero, den_cons, List.getElem_cons_zero, Nat.mul_zero, pow_zero,
        mul_one]
      push_cast; ring
    | succ q =>
      have hq : q < tl.length := by simpa using h
      simp only [List.set_cons_succ, den_cons, List.getElem_cons_succ]
      have hp2 : (2 : ℤ) ^ (64 * (q + 1)) = 2 ^ (64 * q) * 2 ^ 64 := by
        rw [Nat.mul_succ, pow_add]
      push_cast; rw [ih q hq, hp2]; ring

/-- The freshly-allocated `out = [0; k]` denotes `0`. -/
private theorem den_replicate_zero (k : ℕ) : den (List.replicate k 0#u64) = 0 := by
  induction k with
  | zero => rfl
  | succ n ih => rw [List.replicate_succ, den_cons, ih]; simp

set_option maxHeartbeats 1000000 in
/-- `mul`'s inner `j`-loop accumulates one row `ai · v` (starting from `out0`) into `out`, positions
    `i..i+m`. Invariant (over ℤ): `den out + carry·2^(64(i+j)) = den out0 + ai·den(take j v)·2^(64i)`,
    with `carry < 2^64` (bounds the `u128` arithmetic). -/
private theorem mul_loop0_loop0_spec (v out0 out : alloc.vec.Vec Std.U64)
    (i : Std.Usize) (ai carry : Std.U128) (j : Std.Usize)
    (hai : ai.val < 2 ^ 64)
    (hlen : out.val.length = out0.val.length)
    (hcap : i.val + v.val.length ≤ out0.val.length)
    (hcapmax : out0.val.length ≤ Std.Usize.max)
    (hj : j.val ≤ v.val.length)
    (hcarry : carry.val < 2 ^ 64)
    (hinv : (den out.val : ℤ) + carry.val * 2 ^ (64 * (i.val + j.val))
      = (den out0.val : ℤ) + ai.val * (den (v.val.take j.val)) * 2 ^ (64 * i.val)) :
    RefNat.mul_loop0_loop0 v out i carry ai j
      ⦃ r => r.1.val.length = out0.val.length ∧ r.2.val < 2 ^ 64 ∧
          (den r.1.val : ℤ) + r.2.val * 2 ^ (64 * (i.val + v.val.length))
            = (den out0.val : ℤ) + ai.val * (den v.val) * 2 ^ (64 * i.val) ⦄ := by
  unfold RefNat.mul_loop0_loop0
  apply loop.spec_decr_nat
    (measure := fun st => v.val.length - st.2.2.val)
    (inv := fun st => st.2.2.val ≤ v.val.length ∧ st.1.val.length = out0.val.length ∧
      st.2.1.val < 2 ^ 64 ∧
      (den st.1.val : ℤ) + st.2.1.val * 2 ^ (64 * (i.val + st.2.2.val))
        = (den out0.val : ℤ) + ai.val * (den (v.val.take st.2.2.val)) * 2 ^ (64 * i.val))
  · rintro ⟨out', carry', j'⟩ ⟨hj', hlen', hcar', hinv'⟩
    show RefNat.mul_loop0_loop0.body v i ai out' carry' j' ⦃ _ ⦄
    unfold RefNat.mul_loop0_loop0.body
    simp only [] at hj' hlen' hcar' hinv'
    dsimp only []
    have hvlen : (alloc.vec.Vec.len v).val = v.val.length := alloc.vec.Vec.len_val v
    by_cases hlt : j' < alloc.vec.Vec.len v
    · rw [if_pos hlt]
      have hjm : j'.val < v.val.length := by scalar_tac
      have hi2 : i.val + j'.val < out'.val.length := by rw [hlen']; omega
      step; step; step; step; step
      have hi4 : i4.val < 2 ^ 64 := by rw [i4_post, cast_u128_val]; scalar_tac
      have hi6 : i6.val < 2 ^ 64 := by rw [i6_post, cast_u128_val]; scalar_tac
      have hmul : ai.val * i6.val ≤ (2 ^ 64 - 1) * (2 ^ 64 - 1) :=
        Nat.mul_le_mul (by omega) (by omega)
      step; step; step
      step; step; step; step
      -- the freshly-written limb `out'[i2] := cur % 2^64`, and the propagated carry `cur / 2^64`
      have hi2len : i2.val < out'.val.length := by rw [i2_post]; exact hi2
      have hcurZ : (cur.val : ℤ)
          = ((out'.val[i2.val]'hi2len).val : ℤ) + ai.val * (v.val[j'.val]'hjm).val + carry'.val := by
        rw [cur_post, i8_post, i7_post, i4_post, cast_u128_val, i3_post, i6_post, cast_u128_val,
          i5_post]; push_cast; ring
      have hsplitZ : (i9.val : ℤ) + 2 ^ 64 * (carry1.val : ℤ) = (cur.val : ℤ) := by
        rw [i9_post, carry1_post1]; exact_mod_cast u128_split cur
      have htake : (den (v.val.take j1.val) : ℤ)
          = (den (v.val.take j'.val) : ℤ) + 2 ^ (64 * j'.val) * (v.val[j'.val]'hjm).val := by
        rw [j1_post]; exact_mod_cast den_take_succ v.val j'.val hjm
      have hcurlt : cur.val < 2 ^ 64 * 2 ^ 64 := by
        have hc : cur.val = i4.val + ai.val * i6.val + carry'.val := by
          rw [cur_post, i8_post, i7_post]
        rw [hc]; omega
      refine ⟨by scalar_tac, ?_, ?_, ?_, by scalar_tac⟩
      · simp only [__post2, alloc.vec.Vec.set_val_eq, List.length_set, hlen']
      · rw [carry1_post1, Nat.shiftRight_eq_div_pow]
        exact (Nat.div_lt_iff_lt_mul (by positivity)).mpr hcurlt
      · rw [__post2, alloc.vec.Vec.set_val_eq, den_set out'.val i2.val i9 hi2len]
        have hp1 : (2 : ℤ) ^ (64 * (i.val + j1.val)) = 2 ^ (64 * i2.val) * 2 ^ 64 := by
          rw [i2_post, j1_post, show 64 * (i.val + (j'.val + 1)) = 64 * (i.val + j'.val) + 64 from by
            ring, pow_add]
        have hp2 : (2 : ℤ) ^ (64 * i2.val) = 2 ^ (64 * i.val) * 2 ^ (64 * j'.val) := by
          rw [i2_post, show 64 * (i.val + j'.val) = 64 * i.val + 64 * j'.val from by ring, pow_add]
        rw [hp1, htake, hp2]
        rw [show 64 * (i.val + j'.val) = 64 * i.val + 64 * j'.val from by ring, pow_add] at hinv'
        linear_combination hinv' + (2 ^ (64 * i.val) * 2 ^ (64 * j'.val)) * hsplitZ +
          (2 ^ (64 * i.val) * 2 ^ (64 * j'.val)) * hcurZ
    · rw [if_neg hlt]
      simp only [WP.spec_ok]
      have hjm : j'.val = v.val.length := by scalar_tac
      rw [hjm, List.take_length] at hinv'
      exact ⟨hlen', hcar', hinv'⟩
  · exact ⟨hj, hlen, hcarry, hinv⟩

set_option maxHeartbeats 1000000 in
/-- `mul`'s carry-propagation `k`-loop adds the row's final `carry` into the higher limbs. It keeps
    `den out + carry·2^(64k)` invariant, terminating when `carry = 0`. In-bounds (`k < len out`) holds
    because the running value is `< 2^(64·B)` for a limb bound `B ≤ len out`: a nonzero carry forces
    `2^(64k) ≤ value < 2^(64B)`, hence `k < B`. -/
private theorem mul_loop0_loop1_spec (out : alloc.vec.Vec Std.U64) (carry : Std.U128) (k : Std.Usize)
    (B : ℕ) (hB : B ≤ out.val.length) (hcapmax : out.val.length ≤ Std.Usize.max)
    (hcarry : carry.val < 2 ^ 64)
    (hbound : (den out.val : ℤ) + carry.val * 2 ^ (64 * k.val) < 2 ^ (64 * B)) :
    RefNat.mul_loop0_loop1 out carry k
      ⦃ r => (den r.val : ℤ) = (den out.val : ℤ) + carry.val * 2 ^ (64 * k.val)
          ∧ r.val.length = out.val.length ⦄ := by
  unfold RefNat.mul_loop0_loop1
  apply loop.spec_decr_nat
    (measure := fun st => out.val.length - st.2.2.val)
    (inv := fun st => st.2.1.val < 2 ^ 64 ∧ st.1.val.length = out.val.length ∧
      (den st.1.val : ℤ) + st.2.1.val * 2 ^ (64 * st.2.2.val)
        = (den out.val : ℤ) + carry.val * 2 ^ (64 * k.val))
  · rintro ⟨out', carry', k'⟩ ⟨hcar', hlen', hinv'⟩
    show RefNat.mul_loop0_loop1.body out' carry' k' ⦃ _ ⦄
    unfold RefNat.mul_loop0_loop1.body
    simp only [] at hcar' hlen' hinv'
    by_cases hc : carry' != 0#u128
    · rw [if_pos hc]
      have hcpos : carry'.val ≠ 0 := by simp only [bne_iff_ne, ne_eq] at hc; scalar_tac
      have hk'lt : k'.val < out'.val.length := by
        rw [hlen']
        have hpp : (0 : ℤ) < 2 ^ (64 * k'.val) := pow_pos (by norm_num) _
        have hcar1 : (1 : ℤ) ≤ carry'.val := by
          have : 1 ≤ carry'.val := Nat.one_le_iff_ne_zero.mpr hcpos; exact_mod_cast this
        have hle : (2 : ℤ) ^ (64 * k'.val) ≤ (den out.val : ℤ) + carry.val * 2 ^ (64 * k.val) := by
          rw [← hinv']; have h0 : (0 : ℤ) ≤ (den out'.val : ℤ) := Int.natCast_nonneg _; nlinarith
        have hlt : (2 : ℤ) ^ (64 * k'.val) < 2 ^ (64 * B) := lt_of_le_of_lt hle hbound
        have : 64 * k'.val < 64 * B := by
          rcases lt_or_ge (64 * k'.val) (64 * B) with h | h
          · exact h
          · exact absurd hlt (not_lt.mpr (pow_le_pow_right₀ (by norm_num) h))
        omega
      step; step; step
      step; step; step; step
      have hcurZ : (cur.val : ℤ) = ((out'.val[k'.val]'hk'lt).val : ℤ) + carry'.val := by
        rw [cur_post, i1_post, cast_u128_val, i_post]; push_cast; ring
      have hsplitZ : (i2.val : ℤ) + 2 ^ 64 * (carry1.val : ℤ) = (cur.val : ℤ) := by
        rw [i2_post, carry1_post1]; exact_mod_cast u128_split cur
      have hcurlt : cur.val < 2 ^ 64 * 2 ^ 64 := by
        have hc' : cur.val = i1.val + carry'.val := by rw [cur_post]
        have hi1 : i1.val < 2 ^ 64 := by rw [i1_post, cast_u128_val]; scalar_tac
        omega
      refine ⟨?_, ?_, ?_, by omega⟩
      · rw [carry1_post1, Nat.shiftRight_eq_div_pow]
        exact (Nat.div_lt_iff_lt_mul (by positivity)).mpr hcurlt
      · simp only [__post2, alloc.vec.Vec.set_val_eq, List.length_set, hlen']
      · rw [__post2, alloc.vec.Vec.set_val_eq, den_set out'.val k'.val i2 hk'lt]
        have hp1 : (2 : ℤ) ^ (64 * k1.val) = 2 ^ (64 * k'.val) * 2 ^ 64 := by
          rw [k1_post, show 64 * (k'.val + 1) = 64 * k'.val + 64 from by ring, pow_add]
        rw [hp1, ← hinv']
        linear_combination 2 ^ (64 * k'.val) * hsplitZ + 2 ^ (64 * k'.val) * hcurZ
    · rw [if_neg hc]
      simp only [WP.spec_ok, bne_iff_ne, ne_eq, not_not] at hc ⊢
      have hc0 : carry'.val = 0 := by rw [hc]; rfl
      rw [hc0] at hinv'; simp only [Nat.cast_zero, zero_mul, add_zero] at hinv'
      exact ⟨hinv', hlen'⟩
  · exact ⟨hcarry, rfl, rfl⟩

set_option maxHeartbeats 1000000 in
/-- `mul`'s outer `i`-loop sums the rows `v[i]·v1` (each placed at offset `i`), maintaining
    `den out = den(take i v) · den v1`. Row `i` runs the inner loop then propagates its carry. -/
private theorem mul_loop0_spec (v v1 out : alloc.vec.Vec Std.U64) (i : Std.Usize)
    (hlen : out.val.length = v.val.length + v1.val.length)
    (hcapmax : v.val.length + v1.val.length ≤ Std.Usize.max)
    (hi : i.val ≤ v.val.length)
    (hinv : (den out.val : ℤ) = (den (v.val.take i.val) : ℤ) * (den v1.val : ℤ)) :
    RefNat.mul_loop0 v v1 out i
      ⦃ r => (den r.val : ℤ) = (den v.val : ℤ) * (den v1.val : ℤ)
          ∧ r.val.length = v.val.length + v1.val.length ⦄ := by
  unfold RefNat.mul_loop0
  apply loop.spec_decr_nat
    (measure := fun st => v.val.length - st.2.val)
    (inv := fun st => st.2.val ≤ v.val.length ∧ st.1.val.length = v.val.length + v1.val.length ∧
      (den st.1.val : ℤ) = (den (v.val.take st.2.val) : ℤ) * (den v1.val : ℤ))
  · rintro ⟨out', i'⟩ ⟨hi', hlen', hinv'⟩
    show RefNat.mul_loop0.body v v1 out' i' ⦃ _ ⦄
    unfold RefNat.mul_loop0.body
    simp only [] at hi' hlen' hinv'
    have hvlen : (alloc.vec.Vec.len v).val = v.val.length := alloc.vec.Vec.len_val v
    by_cases hlt : i' < alloc.vec.Vec.len v
    · rw [if_pos hlt]
      have hi'lt : i'.val < v.val.length := by scalar_tac
      step; step
      have hai : ai.val < 2 ^ 64 := by rw [ai_post, cast_u128_val]; scalar_tac
      have hinner := mul_loop0_loop0_spec v1 out' out' i' ai 0#u128 0#usize hai rfl
        (by rw [hlen']; omega) (by rw [hlen']; exact hcapmax) (Nat.zero_le _) (by simp) (by simp)
      cases hcase0 : RefNat.mul_loop0_loop0 v1 out' i' 0#u128 ai 0#usize with
      | ok r =>
        obtain ⟨out1, carry⟩ := r
        rw [hcase0] at hinner; simp only [WP.spec_ok] at hinner
        obtain ⟨hlen1, hcar1, hden1⟩ := hinner
        simp only [bind_tc_ok]
        step
        have hk : r.val = i'.val + v1.val.length := by rw [r_post]; simp
        have haiv : (ai.val : ℤ) = ((v.val[i'.val]'hi'lt).val : ℤ) := by
          rw [ai_post, cast_u128_val, i2_post]
        have htake : (den (v.val.take (i'.val + 1)) : ℤ)
            = (den (v.val.take i'.val) : ℤ) + 2 ^ (64 * i'.val) * (v.val[i'.val]'hi'lt).val := by
          exact_mod_cast den_take_succ v.val i'.val hi'lt
        have hval : (den out1.val : ℤ) + carry.val * 2 ^ (64 * r.val)
            = (den (v.val.take (i'.val + 1)) : ℤ) * (den v1.val : ℤ) := by
          rw [hk, hden1, hinv', htake, haiv]; ring
        have houter := mul_loop0_loop1_spec out1 carry r (i'.val + 1 + v1.val.length)
          (by rw [hlen1, hlen']; omega) (by rw [hlen1, hlen']; exact hcapmax) hcar1 (by
            rw [hval]
            have h1 : (den (v.val.take (i'.val + 1)) : ℤ) < 2 ^ (64 * (i'.val + 1)) := by
              exact_mod_cast den_take_lt v.val (i'.val + 1) (by omega)
            have h2 : (den v1.val : ℤ) < 2 ^ (64 * v1.val.length) := by exact_mod_cast den_lt v1.val
            have hprod := mul_lt_mul'' h1 h2 (Int.natCast_nonneg _) (Int.natCast_nonneg _)
            calc (den (v.val.take (i'.val + 1)) : ℤ) * den v1.val
                < 2 ^ (64 * (i'.val + 1)) * 2 ^ (64 * v1.val.length) := hprod
              _ = 2 ^ (64 * (i'.val + 1 + v1.val.length)) := by
                  rw [← pow_add, show 64 * (i'.val + 1) + 64 * v1.val.length
                    = 64 * (i'.val + 1 + v1.val.length) from by ring])
        cases hcase1 : RefNat.mul_loop0_loop1 out1 carry r with
        | ok out2 =>
          rw [hcase1] at houter; simp only [WP.spec_ok] at houter
          obtain ⟨hden2, hlen2⟩ := houter
          simp only [bind_tc_ok]
          step
          refine ⟨by scalar_tac, ?_, ?_, by scalar_tac⟩
          · rw [hlen2, hlen1, hlen']
          · rw [hden2, hval, i4_post]
        | fail e => rw [hcase1] at houter; exact houter.elim
        | div => rw [hcase1] at houter; exact houter.elim
      | fail e => rw [hcase0] at hinner; exact hinner.elim
      | div => rw [hcase0] at hinner; exact hinner.elim
    · rw [if_neg hlt]
      simp only [WP.spec_ok]
      have hin : i'.val = v.val.length := by scalar_tac
      rw [hin, List.take_length] at hinv'
      exact ⟨hinv', hlen'⟩
  · exact ⟨hi, hlen, hinv⟩

/-- **`mul` refinement.** The lifted `RefNat::mul` computes `den self · den o` on the limb denotations
    (for inputs whose combined limb count fits `usize`). Zero operands short-circuit to `0`. -/
theorem mul_eq (self o : RefNat)
    (hcap : self.limbs.val.length + o.limbs.val.length ≤ Std.Usize.max) :
    RefNat.mul self o
      ⦃ r => den r.limbs.val = den self.limbs.val * den o.limbs.val ∧ Normalized r.limbs.val ⦄ := by
  unfold RefNat.mul
  have hz : ∀ x : RefNat, RefNat.is_zero x = ok x.limbs.val.isEmpty := by
    intro x; unfold RefNat.is_zero alloc.vec.Vec.is_empty; rfl
  simp only [hz, bind_tc_ok]
  by_cases hs : self.limbs.val = []
  · simp only [hs, List.isEmpty_nil, if_true]
    unfold RefNat.zero; simp only [WP.spec_ok]
    exact ⟨by show den (alloc.vec.Vec.new Std.U64).val = _; simp, fun h => absurd rfl h⟩
  · have hse : self.limbs.val.isEmpty = false := by simp [hs]
    simp only [hse, Bool.false_eq_true, if_false]
    by_cases ho : o.limbs.val = []
    · simp only [ho, List.isEmpty_nil, if_true]
      unfold RefNat.zero; simp only [WP.spec_ok]
      exact ⟨by show den (alloc.vec.Vec.new Std.U64).val = _; simp, fun h => absurd rfl h⟩
    · have hoe : o.limbs.val.isEmpty = false := by simp [ho]
      simp only [hoe, Bool.false_eq_true, if_false]
      step
      have hfe := alloc.vec.from_elem_spec core.clone.CloneU64 0#u64 i2 (by rfl)
      cases hfec : alloc.vec.from_elem core.clone.CloneU64 0#u64 i2 with
      | ok out =>
        rw [hfec] at hfe; simp only [WP.spec_ok] at hfe
        obtain ⟨hout_val, hout_len⟩ := hfe
        simp only [bind_tc_ok]
        have hloop := mul_loop0_spec self.limbs o.limbs out 0#usize
          (by rw [hout_val, List.length_replicate, i2_post]; simp)
          hcap (Nat.zero_le _)
          (by rw [hout_val, den_replicate_zero]; simp)
        cases hlc : RefNat.mul_loop0 self.limbs o.limbs out 0#usize with
        | ok out1 =>
          rw [hlc] at hloop; simp only [WP.spec_ok] at hloop
          obtain ⟨hden1, hlen1⟩ := hloop
          simp only [bind_tc_ok]
          have hnorm := normalize_den out1
          have hnn := normalize_normalized out1
          cases hnc : lattice.refbackend.normalize out1 with
          | ok out2 =>
            rw [hnc] at hnorm hnn; simp only [WP.spec_ok] at hnorm hnn
            simp only [bind_tc_ok, WP.spec_ok]
            exact ⟨by show den out2.val = _; rw [hnorm]; exact_mod_cast hden1, hnn⟩
          | fail e => rw [hnc] at hnorm; exact hnorm.elim
          | div => rw [hnc] at hnorm; exact hnorm.elim
        | fail e => rw [hlc] at hloop; exact hloop.elim
        | div => rw [hlc] at hloop; exact hloop.elim
      | fail e => rw [hfec] at hfe; exact hfe.elim
      | div => rw [hfec] at hfe; exact hfe.elim

/-! ### `shl1` — `den (shl1 x) = 2 · den x` (the per-limb left shift with carry) -/

/-- OR-ing a free low bit is addition: if `a` is even and `b ≤ 1`, `(a ||| b).val = a + b`. -/
private theorem u64_or_add (a b : Std.U64) (ha : a.val % 2 = 0) (hb : b.val ≤ 1) :
    (a ||| b).val = a.val + b.val := by
  have hbv : (a ||| b).val = a.val ||| b.val := by
    simp only [UScalar.val]
    rw [show (a ||| b).bv = a.bv ||| b.bv from rfl, BitVec.toNat_or]
  rw [hbv]
  have ha2 : 2 ^ 1 * (a.val / 2) = a.val := by rw [pow_one]; omega
  have h := Nat.two_pow_add_eq_or_of_lt (i := 1) (show b.val < 2 ^ 1 by omega) (a.val / 2)
  rw [ha2] at h; exact h.symm

/-- Setting a currently-clear bit `k` of `a` is addition: `a ||| 2^k = a + 2^k`. -/
private theorem nat_or_pow2_add (a k : ℕ) (hbit : Nat.testBit a k = false) :
    a ||| 2 ^ k = a + 2 ^ k := by
  have hlo : a % 2 ^ k < 2 ^ k := Nat.mod_lt _ (by positivity)
  have hmid : a / 2 ^ k % 2 = 0 := by
    have h := hbit; rw [Nat.testBit_eq_decide_div_mod_eq] at h
    simp only [decide_eq_false_iff_not] at h; omega
  have e2 : a / 2 ^ k = 2 * (a / 2 ^ (k + 1)) := by
    rw [pow_succ, ← Nat.div_div_eq_div_mul]; omega
  have hadecomp : a = 2 ^ (k + 1) * (a / 2 ^ (k + 1)) + a % 2 ^ k := by
    conv_lhs => rw [← Nat.div_add_mod a (2 ^ k), e2]
    rw [pow_succ]; ring
  have hor_lo : a % 2 ^ k ||| 2 ^ k = a % 2 ^ k + 2 ^ k := by
    have h := Nat.two_pow_add_eq_or_of_lt (i := k) hlo 1
    rw [mul_one] at h
    rw [Nat.lor_comm, ← h, Nat.add_comm]
  calc a ||| 2 ^ k
      = (2 ^ (k + 1) * (a / 2 ^ (k + 1)) + a % 2 ^ k) ||| 2 ^ k := by rw [← hadecomp]
    _ = (2 ^ (k + 1) * (a / 2 ^ (k + 1)) ||| a % 2 ^ k) ||| 2 ^ k := by
          rw [Nat.two_pow_add_eq_or_of_lt (by omega : a % 2 ^ k < 2 ^ (k + 1))]
    _ = 2 ^ (k + 1) * (a / 2 ^ (k + 1)) ||| (a % 2 ^ k ||| 2 ^ k) := by rw [Nat.lor_assoc]
    _ = 2 ^ (k + 1) * (a / 2 ^ (k + 1)) ||| (a % 2 ^ k + 2 ^ k) := by rw [hor_lo]
    _ = 2 ^ (k + 1) * (a / 2 ^ (k + 1)) + (a % 2 ^ k + 2 ^ k) :=
          (Nat.two_pow_add_eq_or_of_lt (by omega) _).symm
    _ = a + 2 ^ k := by rw [← Nat.add_assoc, ← hadecomp]

/-- OR-ing in a power-of-two bit `c = 2^k` (currently clear in `a`) is addition. -/
private theorem u64_or_pow2_add (a c : Std.U64) (k : ℕ) (hc : c.val = 2 ^ k)
    (hbit : Nat.testBit a.val k = false) :
    (a ||| c).val = a.val + 2 ^ k := by
  have hor : (a ||| c).val = a.val ||| c.val := by
    simp only [UScalar.val]
    rw [show (a ||| c).bv = a.bv ||| c.bv from rfl, BitVec.toNat_or]
  rw [hor, hc, nat_or_pow2_add a.val k hbit]

set_option maxHeartbeats 800000 in
/-- `shl1`'s per-limb left-shift loop: `out[i] = (v[i] << 1) | carry`, `carry' = v[i] >> 63`, so
    `den out + carry·2^(64i) = 2·den(take i v)`. -/
private theorem shl1_loop_spec (v out : alloc.vec.Vec Std.U64) (carry : Std.U64) (i : Std.Usize)
    (hi : i.val ≤ v.val.length) (hlen : out.val.length = i.val) (hcar : carry.val ≤ 1)
    (hcapmax : v.val.length ≤ Std.Usize.max)
    (hinv : den out.val + carry.val * 2 ^ (64 * i.val) = 2 * den (v.val.take i.val)) :
    RefNat.shl1_loop v out carry i
      ⦃ r => den r.1.val + r.2.val * 2 ^ (64 * v.val.length) = 2 * den v.val
          ∧ r.1.val.length = v.val.length ∧ r.2.val ≤ 1 ⦄ := by
  unfold RefNat.shl1_loop
  apply loop.spec_decr_nat
    (measure := fun st => v.val.length - st.2.2.val)
    (inv := fun st => st.2.2.val ≤ v.val.length ∧ st.1.val.length = st.2.2.val ∧ st.2.1.val ≤ 1 ∧
      den st.1.val + st.2.1.val * 2 ^ (64 * st.2.2.val) = 2 * den (v.val.take st.2.2.val))
  · rintro ⟨out', carry', i'⟩ ⟨hi', hlen', hcar', hinv'⟩
    show RefNat.shl1_loop.body v out' carry' i' ⦃ _ ⦄
    unfold RefNat.shl1_loop.body
    simp only [] at hi' hlen' hcar' hinv'
    dsimp only []
    by_cases hlt : i' < alloc.vec.Vec.len v
    · rw [if_pos hlt]
      have hi'lt : i'.val < v.val.length := by scalar_tac
      step; step; step; step; step
      have hv1lt : v1.val < 2 ^ 64 := by scalar_tac
      have hU : (U64.size : ℕ) = 2 ^ 64 := by simp [U64.size, U64.numBits]
      have hi2v : i2.val = 2 * v1.val % 2 ^ 64 := by
        rw [i2_post1, Nat.shiftLeft_eq, pow_one, hU, Nat.mul_comm]
      have hi2even : i2.val % 2 = 0 := by rw [hi2v]; omega
      have hi3 : i3.val = i2.val + carry'.val := by
        rw [i3_post1]; exact u64_or_add i2 carry' hi2even hcar'
      have hcarry1v : carry1.val = v1.val / 2 ^ 63 := by rw [carry1_post1, Nat.shiftRight_eq_div_pow]
      have hcar1le : carry1.val ≤ 1 := by rw [hcarry1v]; omega
      have hsplit : i3.val + 2 ^ 64 * carry1.val = 2 * v1.val + carry'.val := by
        rw [hi3, hi2v, hcarry1v]
        have hd : 2 * v1.val / 2 ^ 64 = v1.val / 2 ^ 63 := by omega
        have hmad := Nat.mod_add_div (2 * v1.val) (2 ^ 64)
        omega
      step
      refine ⟨by scalar_tac, ?_, hcar1le, ?_, by scalar_tac⟩
      · rw [out1_post, List.length_append, hlen', i4_post]; simp
      · have hpow : (2 : ℕ) ^ (64 * i4.val) = 2 ^ (64 * i'.val) * 2 ^ 64 := by
          rw [i4_post, Nat.mul_add, Nat.mul_one, pow_add]
        have hgrp : 2 ^ (64 * i'.val) * i3.val + carry1.val * 2 ^ (64 * i4.val)
            = 2 * (2 ^ (64 * i'.val) * v1.val) + 2 ^ (64 * i'.val) * carry'.val := by
          rw [hpow]
          have hr : 2 ^ (64 * i'.val) * i3.val + carry1.val * (2 ^ (64 * i'.val) * 2 ^ 64)
              = 2 ^ (64 * i'.val) * (i3.val + 2 ^ 64 * carry1.val) := by ring
          rw [hr, hsplit]; ring
        rw [out1_post, den_append, den_singleton, hlen', add_assoc, hgrp, i4_post,
          den_take_succ v.val i'.val hi'lt, ← v1_post]
        have hc := hinv'
        rw [mul_comm carry'.val] at hc
        omega
    · rw [if_neg hlt]
      simp only [WP.spec_ok]
      have hin : i'.val = v.val.length := by scalar_tac
      rw [hin] at hinv' hlen'
      rw [List.take_length] at hinv'
      exact ⟨hinv', hlen', hcar'⟩
  · exact ⟨hi, hlen, hcar, hinv⟩

/-- **`shl1` doubling.** `den (shl1 x) = 2 · den x` on the limb denotation. -/
theorem shl1_eq (x : RefNat) (hcap : x.limbs.val.length + 1 ≤ Std.Usize.max) :
    RefNat.shl1 x ⦃ r => den r.limbs.val = 2 * den x.limbs.val ∧ Normalized r.limbs.val ⦄ := by
  unfold RefNat.shl1
  simp only [alloc.vec.Vec.with_capacity]
  step
  have hloop := shl1_loop_spec x.limbs (alloc.vec.Vec.new Std.U64) 0#u64 0#usize
    (Nat.zero_le _) (by simp) (by simp) (by scalar_tac) (by simp [den])
  cases hcase : RefNat.shl1_loop x.limbs (alloc.vec.Vec.new Std.U64) 0#u64 0#usize with
  | ok r =>
    obtain ⟨out1, carry⟩ := r
    rw [hcase] at hloop; simp only [WP.spec_ok] at hloop
    obtain ⟨hden, hlen1, hcar1⟩ := hloop
    have hout2 : ∀ out2 : alloc.vec.Vec Std.U64, den out2.val = 2 * den x.limbs.val →
        (do let out3 ← lattice.refbackend.normalize out2; ok ({ limbs := out3 } : RefNat))
          ⦃ r => den r.limbs.val = 2 * den x.limbs.val ∧ Normalized r.limbs.val ⦄ := by
      intro out2 hout2den
      have hnorm := normalize_den out2
      have hnn := normalize_normalized out2
      cases hnc : lattice.refbackend.normalize out2 with
      | ok o =>
        rw [hnc] at hnorm hnn; simp only [WP.spec_ok] at hnorm hnn
        simp only [bind_tc_ok, WP.spec_ok]
        exact ⟨by show den o.val = _; rw [hnorm, hout2den], hnn⟩
      | fail e => rw [hnc] at hnorm; exact hnorm.elim
      | div => rw [hnc] at hnorm; exact hnorm.elim
    simp only [bind_tc_ok]
    show (do
        let out2 ← if (carry != 0#u64) = true then out1.push carry else ok out1
        let out3 ← lattice.refbackend.normalize out2
        ok ({ limbs := out3 } : RefNat))
      ⦃ r => den r.limbs.val = 2 * den x.limbs.val ∧ Normalized r.limbs.val ⦄
    by_cases hcz : carry = 0#u64
    · subst hcz
      simp only [bne_self_eq_false, Bool.false_eq_true, if_false, bind_tc_ok]
      apply hout2
      simpa using hden
    · rw [if_pos (by simp only [ne_eq, bne_iff_ne]; exact hcz)]
      step
      apply hout2
      rw [out2_post, den_append, den_singleton, hlen1]
      rw [mul_comm] at hden; omega
  | fail e => rw [hcase] at hloop; exact hloop.elim
  | div => rw [hcase] at hloop; exact hloop.elim

/-! ### `testbit` — reads bit `i` of the limb denotation -/

/-- Bit `64q+r` (`r < 64`) of the limb denotation is bit `r` of limb `q` (the 64-bit-aligned view). -/
private theorem den_testBit_lt (l : List Std.U64) (q r : ℕ) (hq : q < l.length) (hr : r < 64) :
    Nat.testBit (den l) (64 * q + r) = Nat.testBit (l[q]'hq).val r := by
  induction l generalizing q with
  | nil => exact absurd hq (by simp)
  | cons x xs ih =>
    have hb : x.val < 2 ^ 64 := by scalar_tac
    rw [den_cons, show (x.val : ℕ) + 2 ^ 64 * den xs = 2 ^ 64 * den xs + x.val from Nat.add_comm _ _,
      Nat.testBit_two_pow_mul_add (den xs) hb]
    cases q with
    | zero => simp only [Nat.mul_zero, Nat.zero_add, List.getElem_cons_zero, if_pos hr]
    | succ q' =>
      have hq' : q' < xs.length := by simpa using hq
      rw [if_neg (by omega), show 64 * (q' + 1) + r - 64 = 64 * q' + r from by omega,
        List.getElem_cons_succ]
      exact ih q' hq'

/-- **`testbit`.** `testbit self i` returns bit `i` of the limb denotation. -/
theorem testbit_eq (self : RefNat) (i : Std.Usize) :
    RefNat.testbit self i ⦃ b => b = Nat.testBit (den self.limbs.val) i.val ⦄ := by
  unfold RefNat.testbit
  step
  have hlen : (self.limbs.len).val = self.limbs.val.length := alloc.vec.Vec.len_val self.limbs
  by_cases hge : limb ≥ self.limbs.len
  · rw [if_pos hge]
    simp only [WP.spec_ok]
    symm
    apply Nat.testBit_eq_false_of_lt
    have hb : 64 * self.limbs.val.length ≤ i.val := by
      have h1 : self.limbs.val.length ≤ i.val / 64 := by scalar_tac
      omega
    calc den self.limbs.val < 2 ^ (64 * self.limbs.val.length) := den_lt _
      _ ≤ 2 ^ i.val := Nat.pow_le_pow_right (by norm_num) hb
  · rw [if_neg hge]
    have hlt : limb.val < self.limbs.val.length := by scalar_tac
    step; step; step; step
    have hi3lt : i3.val < 64 := by rw [i3_post]; omega
    have hidx : i.val = 64 * limb.val + i3.val := by rw [limb_post, i3_post]; omega
    have hi5v : i5.val = i2.val / 2 ^ i3.val % 2 := by
      have h1 : i5.val = i4.val &&& 1 := by simp [i5_post1, UScalar.val_and]
      rw [h1, Nat.and_one_is_mod, i4_post1, Nat.shiftRight_eq_div_pow]
    rw [hidx, den_testBit_lt self.limbs.val limb.val i3.val hlt hi3lt, ← i2_post,
      Nat.testBit_eq_decide_div_mod_eq, ← hi5v]
    congr 1
    apply propext
    exact ⟨fun h => by simp [h], fun h => UScalar.eq_of_val_eq (by simp [h])⟩

/-! ### `bit_len` — the bit length bounds the denotation -/

/-- **`bit_len` bound.** `den self < 2^(bit_len self)` — the value fits in `bit_len` bits. -/
theorem bit_len_spec (self : RefNat) (hcap : self.limbs.val.length * 64 ≤ Std.Usize.max) :
    RefNat.bit_len self ⦃ n => den self.limbs.val < 2 ^ n.val ⦄ := by
  unfold RefNat.bit_len
  have hz : RefNat.is_zero self = ok self.limbs.val.isEmpty := by
    unfold RefNat.is_zero alloc.vec.Vec.is_empty; rfl
  rw [hz]
  simp only [bind_tc_ok]
  by_cases he : self.limbs.val = []
  · simp only [he, List.isEmpty_nil, if_true, WP.spec_ok]
    simp [den]
  · have hne : self.limbs.val.isEmpty = false := by simp [he]
    simp only [hne, Bool.false_eq_true, if_false]
    have hlen : (self.limbs.len).val = self.limbs.val.length := alloc.vec.Vec.len_val self.limbs
    have hpos : 0 < self.limbs.val.length := List.length_pos_iff.mpr he
    step
    have htop64 : top.val * 64 ≤ Std.Usize.max := by rw [top_post1, hlen]; omega
    step; step; step; step
    have hlzle : BitVec.leadingZeros i2.bv ≤ 64 := by unfold BitVec.leadingZeros; split <;> omega
    have hcoe : ((BitVec.leadingZeros i2.bv : BitVec 32)).toNat = BitVec.leadingZeros i2.bv := by
      rw [show ((BitVec.leadingZeros i2.bv : BitVec 32)) = BitVec.ofNat 32 (BitVec.leadingZeros i2.bv)
          from rfl, BitVec.toNat_ofNat]
      exact Nat.mod_eq_of_lt (by omega)
    have hi4v : i4.val = BitVec.leadingZeros i2.bv := by
      rw [i4_post, i3_post]
      simp only [core.num.U64.leading_zeros, UScalar.cast, UScalar.val, BitVec.toNat_setWidth,
        UScalarTy.numBits, hcoe]
      cases System.Platform.numBits_eq with
      | inl h => rw [h]; omega
      | inr h => rw [h]; omega
    have hi2lt : i2.val < 2 ^ 64 := by scalar_tac
    have hi2b : i2.val < 2 ^ (64 - i4.val) := by
      rw [hi4v]; unfold BitVec.leadingZeros
      by_cases h0 : i2.bv = 0
      · rw [if_pos h0, Nat.sub_self, pow_zero]
        have : i2.val = 0 := by rw [show i2.val = i2.bv.toNat from rfl, h0]; rfl
        omega
      · rw [if_neg h0]
        have hi2ne : i2.val ≠ 0 := by
          rw [show i2.val = i2.bv.toNat from rfl]
          exact fun hc => h0 (BitVec.eq_of_toNat_eq (by rw [hc]; rfl))
        have hlog : Nat.log 2 i2.val < 64 := Nat.log_lt_of_lt_pow hi2ne hi2lt
        rw [show i2.bv.toNat = i2.val from rfl,
          show 64 - (64 - Nat.log 2 i2.val - 1) = Nat.log 2 i2.val + 1 from by omega]
        exact Nat.lt_pow_succ_log_self (by norm_num) i2.val
    step
    · have hi2last : i2 = self.limbs.val.getLast he := by
        rw [i2_post, List.getLast_eq_getElem]; congr 1
      have hkey : den self.limbs.val = den self.limbs.val.dropLast
          + 2 ^ (64 * (self.limbs.val.length - 1)) * i2.val := by
        conv_lhs => rw [← List.dropLast_append_getLast he]
        rw [den_append, List.length_dropLast, den_singleton, ← hi2last]
      have hdrop : den self.limbs.val.dropLast < 2 ^ (64 * (self.limbs.val.length - 1)) := by
        have := den_lt self.limbs.val.dropLast; rwa [List.length_dropLast] at this
      have hadd : i1.val + i5.val ≤ Std.Usize.max := by
        rw [i1_post, top_post1, hlen, i5_post1]; omega
      step
      rw [n_post, i1_post, top_post1, hlen, i5_post1, hkey]
      have hpow : (2 : ℕ) ^ ((self.limbs.val.length - 1) * 64 + (64 - i4.val))
          = 2 ^ (64 * (self.limbs.val.length - 1)) * 2 ^ (64 - i4.val) := by
        rw [← pow_add]; congr 1; omega
      rw [hpow]
      calc den self.limbs.val.dropLast + 2 ^ (64 * (self.limbs.val.length - 1)) * i2.val
          < 2 ^ (64 * (self.limbs.val.length - 1)) * (1 + i2.val) := by rw [Nat.mul_add]; omega
        _ ≤ 2 ^ (64 * (self.limbs.val.length - 1)) * 2 ^ (64 - i4.val) :=
            Nat.mul_le_mul_left _ (by omega)

/-! ### `divrem` — bit-serial MSB-first restoring long division

The loop scans `self` bit-by-bit from the top, maintaining `r` (the running remainder) and `q` (the
quotient bits set so far). Invariant (over ℕ), at step `i`:
`den self = den q · den d + den r · 2^i + (den self mod 2^i)`, with `2^i ∣ den q`, `den r < den d`.
Each step brings in bit `i-1`, doubles-and-adds into `r`, and conditionally subtracts `d` (setting the
quotient bit). At `i = 0` this gives `den self = den q · den d + den r`, `den r < den d`. -/

/-- For normalized `y`, a strictly shorter list denotes a strictly smaller value. -/
private theorem den_lt_of_len_lt (x y : List Std.U64) (hy : Normalized y) (hlt : x.length < y.length) :
    den x < den y := by
  have hxne : y ≠ [] := by rintro rfl; simp at hlt
  have h1 : den x < 2 ^ (64 * x.length) := den_lt x
  have h2 : (2 : ℕ) ^ (64 * x.length) ≤ 2 ^ (64 * (y.length - 1)) :=
    Nat.pow_le_pow_right (by norm_num) (by omega)
  have h3 : 2 ^ (64 * (y.length - 1)) ≤ den y := den_lower y hxne hy
  omega

/-- The top bit of a `2^(k+1)` window: `n % 2^(k+1) = n % 2^k + 2^k · (bit k of n)`. -/
private theorem nat_mod_two_pow_succ (n k : ℕ) :
    n % 2 ^ (k + 1) = n % 2 ^ k + 2 ^ k * (n / 2 ^ k % 2) := by
  rw [pow_succ, Nat.mod_mul]

/-- Split off the low limb: `den l = l[0] + 2^64·den(l.drop 1)`. -/
private theorem den_head (l : List Std.U64) (h : 0 < l.length) :
    den l = (l[0]'h).val + 2 ^ 64 * den (l.drop 1) := by
  cases l with
  | nil => simp at h
  | cons x xs => simp [den_cons]

set_option maxHeartbeats 1600000 in
set_option maxRecDepth 4000 in
/-- `divrem`'s bit-serial loop, at step `i`: reconstructs `self` from the running quotient `q` and
    remainder `r`, `den self = den q·den d + den r·2^i + den self % 2^i`, with `den r < den d` and
    `2^i ∣ den q`. Terminates at `i = 0` with `den self = den q·den d + den r`. -/
private theorem divrem_loop_spec (self d : RefNat) (q : alloc.vec.Vec Std.U64) (r : RefNat)
    (i : Std.Usize) (hd : Normalized d.limbs.val)
    (hdcap : d.limbs.val.length + 1 ≤ Std.Usize.max)
    (hr : Normalized r.limbs.val) (hrlt : den r.limbs.val < den d.limbs.val)
    (hidvd : 2 ^ i.val ∣ den q.val) (hqcap : i.val ≤ q.val.length * 64)
    (hinv : den self.limbs.val
      = den q.val * den d.limbs.val + den r.limbs.val * 2 ^ i.val + den self.limbs.val % 2 ^ i.val) :
    RefNat.divrem_loop self d q r i
      ⦃ result => den self.limbs.val = den result.1.val * den d.limbs.val + den result.2.limbs.val
          ∧ den result.2.limbs.val < den d.limbs.val ∧ Normalized result.2.limbs.val ⦄ := by
  unfold RefNat.divrem_loop
  apply loop.spec_decr_nat
    (measure := fun st => st.2.2.val)
    (inv := fun st => Normalized st.2.1.limbs.val ∧ den st.2.1.limbs.val < den d.limbs.val ∧
      2 ^ st.2.2.val ∣ den st.1.val ∧ st.2.2.val ≤ st.1.val.length * 64 ∧
      den self.limbs.val = den st.1.val * den d.limbs.val + den st.2.1.limbs.val * 2 ^ st.2.2.val
        + den self.limbs.val % 2 ^ st.2.2.val)
  · rintro ⟨q', r', i'⟩ ⟨hr', hrlt', hidvd', hqcap', hinv'⟩
    show RefNat.divrem_loop.body self d q' r' i' ⦃ _ ⦄
    unfold RefNat.divrem_loop.body
    simp only [] at hr' hrlt' hidvd' hqcap' hinv'
    by_cases hi : i' > 0#usize
    · rw [if_pos hi]
      have hrlen : r'.limbs.val.length ≤ d.limbs.val.length := by
        by_contra hc; have := den_lt_of_len_lt d.limbs.val r'.limbs.val hr' (by omega); omega
      step
      have hshl := shl1_eq r' (by omega)
      cases hsc : RefNat.shl1 r' with
      | ok r1 =>
        rw [hsc] at hshl; simp only [WP.spec_ok] at hshl
        obtain ⟨hr1den, hr1norm⟩ := hshl
        simp only [bind_tc_ok]
        have htb := testbit_eq self i1
        cases htc : RefNat.testbit self i1 with
        | ok b =>
          rw [htc] at htb; simp only [WP.spec_ok] at htb
          simp only [bind_tc_ok]
          have hr2spec : (if b = true then do
                let b1 ← alloc.vec.Vec.is_empty Global r1.limbs
                let v ← if b1 = true then r1.limbs.push 1#u64
                  else do
                    let (i2, index_mut_back) ←
                      alloc.vec.Vec.index_mut (core.slice.index.SliceIndexUsizeSlice Std.U64) r1.limbs 0#usize
                    let i3 ← lift (i2 ||| 1#u64)
                    ok (index_mut_back i3)
                ok ({ limbs := v } : RefNat)
              else ok r1)
              ⦃ r2 => den r2.limbs.val = 2 * den r'.limbs.val + b.toNat
                  ∧ Normalized r2.limbs.val ⦄ := by
            by_cases hb : b = true
            · rw [if_pos hb, hb]
              unfold alloc.vec.Vec.is_empty
              simp only [bind_tc_ok]
              have hr1even : den r1.limbs.val % 2 = 0 := by rw [hr1den]; omega
              by_cases he1 : r1.limbs.val.isEmpty = true
              · rw [if_pos he1]
                have hemp : r1.limbs.val = [] := by simpa using he1
                have hr'0 : den r'.limbs.val = 0 := by
                  rw [hemp] at hr1den; simpa [den] using hr1den.symm
                step
                constructor <;> simp [v_post, hemp, Normalized, hr'0]
              · rw [if_neg he1]
                have hne1 : r1.limbs.val ≠ [] := by simpa using he1
                have hlen1pos : 0 < r1.limbs.val.length := List.length_pos_of_ne_nil hne1
                have hr10 : (r1.limbs.val[0]'hlen1pos).val % 2 = 0 := by
                  have hdh := den_head r1.limbs.val hlen1pos; omega
                have h0 : ((0#usize : Std.Usize).val) = 0 := by rfl
                step
                obtain ⟨i2, imb⟩ := v
                have himb : imb = r1.limbs.set 0#usize := v_post2
                have hi2eq : i2 = (r1.limbs.val[0]'hlen1pos) := v_post1
                step
                have hvval : v.val = (r1.limbs.val[0]'hlen1pos).val + 1 := by
                  have h1 : v.val = (i2 ||| 1#u64).val := v_post1
                  rw [h1, u64_or_add i2 1#u64 (by rw [hi2eq]; exact hr10) (by decide), hi2eq]; rfl
                have hlen_set : (r1.limbs.val.set 0 v).length = r1.limbs.val.length :=
                  List.length_set ..
                rw [himb]
                simp only [alloc.vec.Vec.set_val_eq, h0, Bool.toNat_true]
                refine ⟨?_, ?_⟩
                · have key : (den (r1.limbs.val.set 0 v) : ℤ) = 2 * den r'.limbs.val + 1 := by
                    rw [den_set r1.limbs.val 0 v hlen1pos]
                    push_cast [hvval, hr1den]; ring
                  exact_mod_cast key
                · intro hh
                  rw [List.getLast_eq_getElem, List.getElem_set]
                  split
                  · rw [hvval]; omega
                  · rename_i hne
                    have hn := hr1norm hne1
                    rw [List.getLast_eq_getElem] at hn
                    simpa only [hlen_set] using hn
            · rw [if_neg hb]
              simp only [WP.spec_ok]
              have hb0 : b = false := by simpa using hb
              rw [hb0]; simp only [Bool.toNat_false, add_zero]
              exact ⟨hr1den, hr1norm⟩
          apply WP.spec_bind' hr2spec
          rintro r2 ⟨hr2den, hr2norm⟩
          beta_reduce
          rw [cmp_eq r2 d hr2norm hd]
          simp only [core.cmp.PartialEq.ne.trait_default, core.cmp.PartialEq.ne.default,
            core.cmp.Ordering.Insts.CoreCmpPartialEqOrdering.eq, compare_lt_iff_lt,
            bind_tc_ok, decide_eq_true_eq]
          have hi'1 : i'.val = i1.val + 1 := by omega
          have hble : b.toNat ≤ 1 := by cases b <;> simp
          have hbtoNat : b.toNat = den self.limbs.val / 2 ^ i1.val % 2 := by
            rw [htb, Nat.testBit_eq_decide_div_mod_eq]
            by_cases hc : den self.limbs.val / 2 ^ i1.val % 2 = 1
            · rw [hc]; simp
            · have h0' : den self.limbs.val / 2 ^ i1.val % 2 = 0 := by omega
              rw [h0']; simp
          have hmod : den self.limbs.val % 2 ^ i'.val
              = den self.limbs.val % 2 ^ i1.val + 2 ^ i1.val * b.toNat := by
            rw [hi'1, nat_mod_two_pow_succ, ← hbtoNat]
          have hrecon2 : den self.limbs.val
              = den q'.val * den d.limbs.val + den r2.limbs.val * 2 ^ i1.val
                + den self.limbs.val % 2 ^ i1.val := by
            have hp : (2 : ℕ) ^ i'.val = 2 * 2 ^ i1.val := by rw [hi'1, pow_succ]; ring
            have h := hinv'
            rw [hmod, hp] at h
            rw [hr2den]; conv_lhs => rw [h]
            ring
          have hidvd1 : 2 ^ i1.val ∣ den q'.val :=
            dvd_trans (pow_dvd_pow 2 (by omega)) hidvd'
          by_cases hlt2 : den r2.limbs.val < den d.limbs.val
          · rw [if_neg (not_not_intro hlt2)]
            simp only [WP.spec_ok]
            exact ⟨hr2norm, hlt2, hidvd1, by omega, hrecon2, by omega⟩
          · rw [if_pos hlt2]
            have hge : den d.limbs.val ≤ den r2.limbs.val := by omega
            have hlolen : d.limbs.val.length ≤ r2.limbs.val.length := by
              by_contra hc; rw [not_le] at hc
              exact absurd (den_lt_of_len_lt r2.limbs.val d.limbs.val hd hc) (by omega)
            have hsub := sub_eq r2 d hlolen hge
            cases hsc2 : RefNat.sub r2 d with
            | ok r3 =>
              rw [hsc2] at hsub; simp only [WP.spec_ok] at hsub
              obtain ⟨hr3den, hr3norm⟩ := hsub
              simp only [bind_tc_ok]
              step; step; step
              have hi4lt : i4.val < q'.val.length := by rw [i4_post]; omega
              have hi3v : i3.val = 2 ^ (i1.val % 64) := by
                have hU : U64.size = 2 ^ 64 := by simp [U64.size, U64.numBits]
                rw [i3_post1, i2_post, Nat.shiftLeft_eq, one_mul, hU,
                  Nat.mod_eq_of_lt (Nat.pow_lt_pow_right (by norm_num) (by omega : i1.val % 64 < 64))]
              have hqbit : Nat.testBit (den q'.val) i1.val = false := by
                have hdvd : 2 ^ (i1.val + 1) ∣ den q'.val := by rw [← hi'1]; exact hidvd'
                obtain ⟨m, hm⟩ := hdvd
                rw [Nat.testBit_eq_decide_div_mod_eq, decide_eq_false_iff_not, hm, pow_succ,
                  mul_assoc, Nat.mul_div_cancel_left _ (by positivity : (0:ℕ) < 2 ^ i1.val)]
                omega
              have hqbit4 : Nat.testBit (q'.val[i4.val]'hi4lt).val (i1.val % 64) = false := by
                have hde := den_testBit_lt q'.val i4.val (i1.val % 64) hi4lt (by omega)
                have hidx : 64 * i4.val + i1.val % 64 = i1.val := by rw [i4_post]; omega
                rw [hidx, hqbit] at hde; exact hde.symm
              step
              step
              have hi6v : i6.val = (q'.val[i4.val]'hi4lt).val + 2 ^ (i1.val % 64) := by
                rw [i6_post1, u64_or_pow2_add i5 i3 (i1.val % 64) hi3v
                  (by rw [i5_post1]; exact hqbit4), i5_post1]
              have hpow : (2 : ℤ) ^ (i1.val % 64) * 2 ^ (64 * i4.val) = 2 ^ i1.val := by
                rw [← pow_add, i4_post]; congr 1; omega
              have hq1den : (den (↑(index_mut_back i6) : List Std.U64) : ℤ)
                  = den q'.val + 2 ^ i1.val := by
                rw [i5_post2, alloc.vec.Vec.set_val_eq, den_set q'.val i4.val i6 hi4lt, hi6v]
                push_cast; rw [← hpow]; ring
              have hq1dennat : den (↑(index_mut_back i6) : List Std.U64) = den q'.val + 2 ^ i1.val := by
                exact_mod_cast hq1den
              have hq1len : (↑(index_mut_back i6) : List Std.U64).length = q'.val.length := by
                rw [i5_post2, alloc.vec.Vec.set_val_eq, List.length_set]
              refine ⟨hr3norm, ?_, ?_, ?_, ?_, by omega⟩
              · rw [hr3den]; omega
              · rw [hq1dennat]; exact Dvd.dvd.add hidvd1 (dvd_refl _)
              · rw [hq1len]; omega
              · rw [hq1dennat, hr3den]
                have e2 : (den r2.limbs.val - den d.limbs.val) * 2 ^ i1.val
                    = den r2.limbs.val * 2 ^ i1.val - den d.limbs.val * 2 ^ i1.val := Nat.sub_mul _ _ _
                have e3 : (den q'.val + 2 ^ i1.val) * den d.limbs.val
                    = den q'.val * den d.limbs.val + 2 ^ i1.val * den d.limbs.val := Nat.add_mul _ _ _
                have e1 : den d.limbs.val * 2 ^ i1.val = 2 ^ i1.val * den d.limbs.val := Nat.mul_comm _ _
                have hle2 : den d.limbs.val * 2 ^ i1.val ≤ den r2.limbs.val * 2 ^ i1.val :=
                  Nat.mul_le_mul_right _ hge
                rw [e3, e2]; omega
            | fail e => rw [hsc2] at hsub; exact hsub.elim
            | div => rw [hsc2] at hsub; exact hsub.elim
        | fail e => rw [htc] at htb; exact htb.elim
        | div => rw [htc] at htb; exact htb.elim
      | fail e => rw [hsc] at hshl; exact hshl.elim
      | div => rw [hsc] at hshl; exact hshl.elim
    · rw [if_neg hi]
      simp only [WP.spec_ok]
      have hi0 : i'.val = 0 := by clear hinv' hidvd' hqcap' hrlt' hr'; scalar_tac
      rw [hi0, pow_zero, mul_one, Nat.mod_one, add_zero] at hinv'
      exact ⟨hinv', hrlt', hr'⟩
  · exact ⟨hr, hrlt, hidvd, hqcap, hinv⟩

set_option maxHeartbeats 800000 in
/-- **`divrem` refinement.** For a nonzero normalized divisor, the bit-serial long division computes
    the Euclidean quotient and remainder on the limb denotations. -/
theorem divrem_eq (self d : RefNat) (hself : Normalized self.limbs.val)
    (hd : Normalized d.limbs.val) (hdpos : 0 < den d.limbs.val)
    (hdcap : d.limbs.val.length + 1 ≤ Std.Usize.max)
    (hcap : self.limbs.val.length * 64 ≤ Std.Usize.max) :
    RefNat.divrem self d
      ⦃ r => den r.1.limbs.val = den self.limbs.val / den d.limbs.val
          ∧ den r.2.limbs.val = den self.limbs.val % den d.limbs.val
          ∧ Normalized r.1.limbs.val ∧ Normalized r.2.limbs.val ⦄ := by
  unfold RefNat.divrem
  rw [is_zero_eq d hd]
  have hb0 : decide (den d.limbs.val = 0) = false := by
    simp only [decide_eq_false_iff_not]; omega
  simp only [hb0, bind_tc_ok, massert]
  rw [cmp_eq self d hself hd]
  simp only [core.cmp.Ordering.Insts.CoreCmpPartialEqOrdering.eq, compare_lt_iff_lt,
    bind_tc_ok, decide_eq_true_eq]
  rw [if_pos (by decide : ¬ (false = true))]
  simp only [bind_tc_ok]
  by_cases hlt : den self.limbs.val < den d.limbs.val
  · -- `self < d`: quotient `0`, remainder `self`.
    rw [if_pos hlt]
    unfold RefNat.zero RefNat.Insts.CoreCloneClone.clone
    have hcl : alloc.vec.CloneVec.clone core.clone.CloneU64 self.limbs ⦃ v => self.limbs = v ⦄ :=
      alloc.slice.Slice.to_vec_spec core.clone.CloneU64 self.limbs (by intro x _; rfl)
    cases hcc : alloc.vec.CloneVec.clone core.clone.CloneU64 self.limbs with
    | ok v =>
      rw [hcc] at hcl; simp only [WP.spec_ok] at hcl
      simp only [bind_tc_ok, WP.spec_ok]
      refine ⟨?_, ?_, ?_, ?_⟩
      · show den (alloc.vec.Vec.new Std.U64).val = den self.limbs.val / den d.limbs.val
        rw [Nat.div_eq_of_lt hlt]; rfl
      · show den v.val = den self.limbs.val % den d.limbs.val
        rw [← hcl, Nat.mod_eq_of_lt hlt]
      · show Normalized (alloc.vec.Vec.new Std.U64).val
        intro h; exact absurd rfl h
      · show Normalized v.val
        rw [← hcl]; exact hself
    | fail e => rw [hcc] at hcl; exact hcl.elim
    | div => rw [hcc] at hcl; exact hcl.elim
  · -- `self ≥ d`: the bit-serial loop, over `⌈bit_len self / 64⌉` limbs.
    rw [if_neg hlt]
    have hbl := bit_len_spec self hcap
    cases hbn : RefNat.bit_len self with
    | ok n =>
      rw [hbn] at hbl; simp only [WP.spec_ok] at hbl
      simp only [bind_tc_ok, core.num.Usize.div_ceil]
      step
      unfold RefNat.zero
      simp only [bind_tc_ok]
      have hq0 : den q.val = 0 := by rw [q_post1]; exact den_replicate_zero _
      have hqlen : q.val.length = (n.val + 64 - 1) / 64 := by simpa using q_post2
      have hloop := divrem_loop_spec self d q { limbs := alloc.vec.Vec.new Std.U64 } n hd hdcap
        (by intro h; exact absurd rfl h)
        (by show den (alloc.vec.Vec.new Std.U64).val < den d.limbs.val; rw [show den (alloc.vec.Vec.new Std.U64).val = 0 from rfl]; exact hdpos)
        (by rw [hq0]; exact dvd_zero _)
        (by rw [hqlen]; omega)
        (by rw [hq0, show den ({ limbs := alloc.vec.Vec.new Std.U64 } : RefNat).limbs.val = 0 from rfl,
              Nat.mod_eq_of_lt hbl]; ring)
      cases hlc : RefNat.divrem_loop self d q { limbs := alloc.vec.Vec.new Std.U64 } n with
      | ok qr =>
        obtain ⟨q1, r1⟩ := qr
        rw [hlc] at hloop; simp only [WP.spec_ok] at hloop
        obtain ⟨hrecon, hrlt2, hrnorm⟩ := hloop
        show (do let q2 ← lattice.refbackend.normalize q1
                 ok ((⟨q2⟩ : RefNat), r1))
          ⦃ r => den r.1.limbs.val = den self.limbs.val / den d.limbs.val
              ∧ den r.2.limbs.val = den self.limbs.val % den d.limbs.val
              ∧ Normalized r.1.limbs.val ∧ Normalized r.2.limbs.val ⦄
        have hnd := normalize_den q1
        have hnn := normalize_normalized q1
        cases hnc : lattice.refbackend.normalize q1 with
        | ok q2 =>
          rw [hnc] at hnd hnn; simp only [WP.spec_ok] at hnd hnn
          simp only [bind_tc_ok, WP.spec_ok]
          have hunique : den self.limbs.val / den d.limbs.val = den q1.val
              ∧ den self.limbs.val % den d.limbs.val = den r1.limbs.val := by
            apply (Nat.div_mod_unique hdpos).mpr
            exact ⟨by rw [Nat.mul_comm]; omega, hrlt2⟩
          exact ⟨by rw [hnd]; exact hunique.1.symm, hunique.2.symm, hnn, hrnorm⟩
        | fail e => rw [hnc] at hnd; exact hnd.elim
        | div => rw [hnc] at hnd; exact hnd.elim
      | fail e => rw [hlc] at hloop; exact hloop.elim
      | div => rw [hlc] at hloop; exact hloop.elim
    | fail e => rw [hbn] at hbl; exact hbl.elim
    | div => rw [hbn] at hbl; exact hbl.elim

/-! ### `gcd` — Euclid's algorithm on the limb denotations -/

set_option maxHeartbeats 800000 in
/-- `gcd`'s Euclid loop: `while y ≠ 0 do (x, y) := (y, x % y)`, returning `x`. Maintains
    `gcd (den x) (den y)` invariant, terminating (measure `den y`) at `y = 0` with `gcd (den x) 0`. -/
private theorem gcd_loop_spec (x y : RefNat) (hx : Normalized x.limbs.val) (hy : Normalized y.limbs.val)
    (hxcap : x.limbs.val.length * 64 ≤ Std.Usize.max)
    (hycap : y.limbs.val.length * 64 ≤ Std.Usize.max) :
    RefNat.gcd_loop x y
      ⦃ r => den r.limbs.val = Nat.gcd (den x.limbs.val) (den y.limbs.val)
          ∧ Normalized r.limbs.val ⦄ := by
  unfold RefNat.gcd_loop
  apply loop.spec_decr_nat
    (measure := fun st => den st.2.limbs.val)
    (inv := fun st => Normalized st.1.limbs.val ∧ Normalized st.2.limbs.val ∧
      st.1.limbs.val.length * 64 ≤ Std.Usize.max ∧ st.2.limbs.val.length * 64 ≤ Std.Usize.max ∧
      Nat.gcd (den st.1.limbs.val) (den st.2.limbs.val) = Nat.gcd (den x.limbs.val) (den y.limbs.val))
  · rintro ⟨x', y'⟩ ⟨hx', hy', hxcap', hycap', hgcd⟩
    show RefNat.gcd_loop.body x' y' ⦃ _ ⦄
    unfold RefNat.gcd_loop.body
    simp only [] at hx' hy' hxcap' hycap' hgcd
    rw [is_zero_eq y' hy']
    simp only [bind_tc_ok]
    by_cases hyz : den y'.limbs.val = 0
    · rw [if_pos (by simp [hyz])]
      simp only [WP.spec_ok]
      refine ⟨?_, hx'⟩
      rw [← hgcd, hyz, Nat.gcd_zero_right]
    · rw [if_neg (by simp [hyz])]
      have hypos : 0 < den y'.limbs.val := by omega
      have hyne : y'.limbs.val ≠ [] := fun h => hyz ((den_eq_zero_iff y'.limbs.val hy').mpr h)
      have hylen1 : 1 ≤ y'.limbs.val.length := List.length_pos_of_ne_nil hyne
      have hdcap : y'.limbs.val.length + 1 ≤ Std.Usize.max := by nlinarith [hycap', hylen1]
      have hdr := divrem_eq x' y' hx' hy' hypos hdcap hxcap'
      cases hdc : RefNat.divrem x' y' with
      | ok qr =>
        obtain ⟨q1, r1⟩ := qr
        rw [hdc] at hdr; simp only [WP.spec_ok] at hdr
        obtain ⟨hq, hrmod, hqnorm, hrnorm⟩ := hdr
        simp only [bind_tc_ok]
        have hrlt : den r1.limbs.val < den y'.limbs.val := by rw [hrmod]; exact Nat.mod_lt _ hypos
        have hrlen : r1.limbs.val.length ≤ y'.limbs.val.length := by
          by_contra hc; rw [not_le] at hc
          exact absurd (den_lt_of_len_lt y'.limbs.val r1.limbs.val hrnorm hc) (by omega)
        refine ⟨⟨hy', hrnorm, hycap', by nlinarith [hrlen, hycap'], ?_⟩, hrlt⟩
        rw [hrmod, Nat.gcd_comm (den y'.limbs.val), ← Nat.gcd_rec, Nat.gcd_comm]
        exact hgcd
      | fail e => rw [hdc] at hdr; exact hdr.elim
      | div => rw [hdc] at hdr; exact hdr.elim
  · exact ⟨hx, hy, hxcap, hycap, rfl⟩

/-- **`gcd` refinement.** For normalized inputs, `RefNat::gcd` computes `Nat.gcd` on the denotations. -/
theorem gcd_eq (self o : RefNat) (hself : Normalized self.limbs.val) (ho : Normalized o.limbs.val)
    (hscap : self.limbs.val.length * 64 ≤ Std.Usize.max)
    (hocap : o.limbs.val.length * 64 ≤ Std.Usize.max) :
    RefNat.gcd self o
      ⦃ r => den r.limbs.val = Nat.gcd (den self.limbs.val) (den o.limbs.val)
          ∧ Normalized r.limbs.val ⦄ := by
  unfold RefNat.gcd RefNat.Insts.CoreCloneClone.clone
  have hcx : alloc.vec.CloneVec.clone core.clone.CloneU64 self.limbs ⦃ v => self.limbs = v ⦄ :=
    alloc.slice.Slice.to_vec_spec core.clone.CloneU64 self.limbs (by intro x _; rfl)
  cases hccx : alloc.vec.CloneVec.clone core.clone.CloneU64 self.limbs with
  | ok vx =>
    rw [hccx] at hcx; simp only [WP.spec_ok] at hcx
    simp only [bind_tc_ok]
    have hcy : alloc.vec.CloneVec.clone core.clone.CloneU64 o.limbs ⦃ v => o.limbs = v ⦄ :=
      alloc.slice.Slice.to_vec_spec core.clone.CloneU64 o.limbs (by intro x _; rfl)
    cases hccy : alloc.vec.CloneVec.clone core.clone.CloneU64 o.limbs with
    | ok vy =>
      rw [hccy] at hcy; simp only [WP.spec_ok] at hcy
      simp only [bind_tc_ok]
      have hvx : (⟨vx⟩ : RefNat).limbs.val = self.limbs.val := by rw [← hcx]
      have hvy : (⟨vy⟩ : RefNat).limbs.val = o.limbs.val := by rw [← hcy]
      have hloop := gcd_loop_spec ⟨vx⟩ ⟨vy⟩ (by rw [hvx]; exact hself) (by rw [hvy]; exact ho)
        (by rw [hvx]; exact hscap) (by rw [hvy]; exact hocap)
      rw [hvx, hvy] at hloop
      exact hloop
    | fail e => rw [hccy] at hcy; exact hcy.elim
    | div => rw [hccy] at hcy; exact hcy.elim
  | fail e => rw [hccx] at hcx; exact hcx.elim
  | div => rw [hccx] at hcx; exact hcx.elim

/-! ## `RefInt` → ℤ — sign-magnitude integers on top of the `RefNat` layer

`iden a = ±den(a.mag)`. Canonical form `IntNorm`: normalized magnitude, sign-of-zero pinned to
`false` (so equal integers have equal representations). Every constructor goes through `make`. -/

/-- Signed denotation of a `RefInt`: `+den(mag)` or `−den(mag)` per the sign bit. -/
def iden (a : RefInt) : ℤ := if a.neg then -(den a.mag.limbs.val : ℤ) else (den a.mag.limbs.val : ℤ)

/-- Canonical `RefInt`: normalized magnitude, and the sign of zero pinned to `false`. -/
def IntNorm (a : RefInt) : Prop :=
  Normalized a.mag.limbs.val ∧ (den a.mag.limbs.val = 0 → a.neg = false)

/-- **`make` refinement.** `make neg mag = ±den(mag)` (canonicalizing the sign of `0`). -/
private theorem make_spec (neg : Bool) (mag : RefNat) (hmag : Normalized mag.limbs.val) :
    RefInt.make neg mag
      ⦃ r => iden r = (if neg then -(den mag.limbs.val : ℤ) else (den mag.limbs.val : ℤ))
          ∧ IntNorm r ∧ r.mag = mag ⦄ := by
  unfold RefInt.make
  rw [is_zero_eq mag hmag]
  simp only [bind_tc_ok]
  by_cases hz : den mag.limbs.val = 0
  · rw [if_pos (by simp [hz])]
    simp only [WP.spec_ok]
    refine ⟨?_, ⟨hmag, fun _ => rfl⟩, trivial⟩
    cases neg <;> simp [iden, hz]
  · rw [if_neg (by simp [hz])]
    simp only [WP.spec_ok]
    exact ⟨rfl, ⟨hmag, fun h => absurd h hz⟩, trivial⟩

/-- **`RefInt.zero`** denotes `0`. -/
private theorem int_zero_eq : RefInt.zero ⦃ r => iden r = 0 ∧ IntNorm r ⦄ := by
  unfold RefInt.zero RefNat.zero
  simp only [bind_tc_ok, WP.spec_ok]
  exact ⟨by simp [iden], fun h => absurd rfl h, fun _ => rfl⟩

/-- **`RefInt.mul`** denotes `iden a · iden b`. -/
private theorem int_mul_eq (a b : RefInt)
    (hcap : a.mag.limbs.val.length + b.mag.limbs.val.length ≤ Std.Usize.max) :
    RefInt.mul a b ⦃ r => iden r = iden a * iden b ∧ IntNorm r ⦄ := by
  unfold RefInt.mul
  have hmul := mul_eq a.mag b.mag hcap
  cases hmc : RefNat.mul a.mag b.mag with
  | ok m =>
    rw [hmc] at hmul; simp only [WP.spec_ok] at hmul
    obtain ⟨hmden, hmnorm⟩ := hmul
    simp only [bind_tc_ok]
    have hmk := make_spec (a.neg != b.neg) m hmnorm
    cases hmkc : RefInt.make (a.neg != b.neg) m with
    | ok r =>
      rw [hmkc] at hmk; simp only [WP.spec_ok] at hmk
      obtain ⟨hrden, hrnorm, _⟩ := hmk
      simp only [WP.spec_ok]
      refine ⟨?_, hrnorm⟩
      rw [hrden, hmden]
      simp only [iden]
      cases a.neg <;> cases b.neg <;> simp
    | fail e => rw [hmkc] at hmk; exact hmk.elim
    | div => rw [hmkc] at hmk; exact hmk.elim
  | fail e => rw [hmc] at hmul; exact hmul.elim
  | div => rw [hmc] at hmul; exact hmul.elim

/-- **`RefInt.neg`** denotes `-iden a` (and preserves the magnitude — it only flips the sign). -/
private theorem int_neg_eq (a : RefInt) (ha : IntNorm a) :
    RefInt.impl.neg a
      ⦃ r => iden r = -(iden a) ∧ IntNorm r ∧ r.mag.limbs.val = a.mag.limbs.val ⦄ := by
  unfold RefInt.impl.neg RefNat.Insts.CoreCloneClone.clone
  have hcl : alloc.vec.CloneVec.clone core.clone.CloneU64 a.mag.limbs ⦃ v => a.mag.limbs = v ⦄ :=
    alloc.slice.Slice.to_vec_spec core.clone.CloneU64 a.mag.limbs (by intro x _; rfl)
  cases hcc : alloc.vec.CloneVec.clone core.clone.CloneU64 a.mag.limbs with
  | ok v =>
    rw [hcc] at hcl; simp only [WP.spec_ok] at hcl
    simp only [bind_tc_ok]
    have hvden : den (⟨v⟩ : RefNat).limbs.val = den a.mag.limbs.val := by rw [← hcl]
    have hvnorm : Normalized (⟨v⟩ : RefNat).limbs.val := hcl ▸ ha.1
    have hmk := make_spec (¬a.neg) ⟨v⟩ hvnorm
    cases hmkc : RefInt.make (¬a.neg) ⟨v⟩ with
    | ok r =>
      rw [hmkc] at hmk; simp only [WP.spec_ok] at hmk
      obtain ⟨hrden, hrnorm, hrmag⟩ := hmk
      simp only [WP.spec_ok]
      refine ⟨?_, hrnorm, by rw [hrmag, ← hcl]⟩
      rw [hrden, hvden]
      simp only [iden]
      cases a.neg <;> simp
    | fail e => rw [hmkc] at hmk; exact hmk.elim
    | div => rw [hmkc] at hmk; exact hmk.elim
  | fail e => rw [hcc] at hcl; exact hcl.elim
  | div => rw [hcc] at hcl; exact hcl.elim

set_option maxHeartbeats 800000 in
/-- **`RefInt.add`** denotes `iden a + iden b` (sign-magnitude: same sign adds, opposite subtracts
    the smaller magnitude from the larger). -/
private theorem int_add_eq (a b : RefInt) (ha : IntNorm a) (hb : IntNorm b)
    (hcap : max a.mag.limbs.val.length b.mag.limbs.val.length + 1 ≤ Std.Usize.max) :
    RefInt.add a b ⦃ r => iden r = iden a + iden b ∧ IntNorm r ⦄ := by
  unfold RefInt.add
  by_cases hsign : a.neg = b.neg
  · -- same sign: magnitudes add
    rw [if_pos hsign]
    have hadd := add_eq a.mag b.mag hcap
    cases hac : RefNat.add a.mag b.mag with
    | ok s =>
      rw [hac] at hadd; simp only [WP.spec_ok] at hadd
      obtain ⟨hsden, hsnorm⟩ := hadd
      simp only [bind_tc_ok]
      have hmk := make_spec a.neg s hsnorm
      cases hmkc : RefInt.make a.neg s with
      | ok r =>
        rw [hmkc] at hmk; simp only [WP.spec_ok] at hmk
        obtain ⟨hrden, hrnorm, _⟩ := hmk
        simp only [WP.spec_ok]
        refine ⟨?_, hrnorm⟩
        rw [hrden, hsden]
        simp only [iden, ← hsign]
        cases a.neg <;> (simp; try omega)
      | fail e => rw [hmkc] at hmk; exact hmk.elim
      | div => rw [hmkc] at hmk; exact hmk.elim
    | fail e => rw [hac] at hadd; exact hadd.elim
    | div => rw [hac] at hadd; exact hadd.elim
  · -- opposite sign: subtract the smaller magnitude from the larger
    rw [if_neg hsign, cmp_eq a.mag b.mag ha.1 hb.1]
    simp only [bind_tc_ok]
    rcases lt_trichotomy (den a.mag.limbs.val) (den b.mag.limbs.val) with hlt | heq | hgt
    · -- |a| < |b|: result = ±(|b| − |a|) with b's sign
      rw [Nat.compare_eq_lt.mpr hlt]
      have hlolen : a.mag.limbs.val.length ≤ b.mag.limbs.val.length := by
        by_contra hc; rw [not_le] at hc
        exact absurd (den_lt_of_len_lt b.mag.limbs.val a.mag.limbs.val ha.1 hc) (by omega)
      have hsub := sub_eq b.mag a.mag hlolen (le_of_lt hlt)
      cases hsc : RefNat.sub b.mag a.mag with
      | ok s =>
        rw [hsc] at hsub; simp only [WP.spec_ok] at hsub
        obtain ⟨hsden, hsnorm⟩ := hsub
        simp only [bind_tc_ok]
        have hmk := make_spec b.neg s hsnorm
        cases hmkc : RefInt.make b.neg s with
        | ok r =>
          rw [hmkc] at hmk; simp only [WP.spec_ok] at hmk
          obtain ⟨hrden, hrnorm, _⟩ := hmk
          simp only [WP.spec_ok]
          refine ⟨?_, hrnorm⟩
          rw [hrden, hsden]
          simp only [iden]
          cases han : a.neg <;> cases hbn : b.neg <;>
            first | exact absurd (han.trans hbn.symm) hsign | (simp; try omega)
        | fail e => rw [hmkc] at hmk; exact hmk.elim
        | div => rw [hmkc] at hmk; exact hmk.elim
      | fail e => rw [hsc] at hsub; exact hsub.elim
      | div => rw [hsc] at hsub; exact hsub.elim
    · -- |a| = |b|: opposite signs cancel to 0
      rw [Nat.compare_eq_eq.mpr heq]
      have hz := int_zero_eq
      cases hzc : RefInt.zero with
      | ok r =>
        rw [hzc] at hz; simp only [WP.spec_ok] at hz
        obtain ⟨hzden, hznorm⟩ := hz
        simp only [WP.spec_ok]
        refine ⟨?_, hznorm⟩
        rw [hzden]
        simp only [iden, heq]
        cases han : a.neg <;> cases hbn : b.neg <;>
          first | exact absurd (han.trans hbn.symm) hsign | (simp; try omega)
      | fail e => rw [hzc] at hz; exact hz.elim
      | div => rw [hzc] at hz; exact hz.elim
    · -- |a| > |b|: result = ±(|a| − |b|) with a's sign
      rw [Nat.compare_eq_gt.mpr hgt]
      have hlolen : b.mag.limbs.val.length ≤ a.mag.limbs.val.length := by
        by_contra hc; rw [not_le] at hc
        exact absurd (den_lt_of_len_lt a.mag.limbs.val b.mag.limbs.val hb.1 hc) (by omega)
      have hsub := sub_eq a.mag b.mag hlolen (le_of_lt hgt)
      cases hsc : RefNat.sub a.mag b.mag with
      | ok s =>
        rw [hsc] at hsub; simp only [WP.spec_ok] at hsub
        obtain ⟨hsden, hsnorm⟩ := hsub
        simp only [bind_tc_ok]
        have hmk := make_spec a.neg s hsnorm
        cases hmkc : RefInt.make a.neg s with
        | ok r =>
          rw [hmkc] at hmk; simp only [WP.spec_ok] at hmk
          obtain ⟨hrden, hrnorm, _⟩ := hmk
          simp only [WP.spec_ok]
          refine ⟨?_, hrnorm⟩
          rw [hrden, hsden]
          simp only [iden]
          cases han : a.neg <;> cases hbn : b.neg <;>
            first | exact absurd (han.trans hbn.symm) hsign | (simp; try omega)
        | fail e => rw [hmkc] at hmk; exact hmk.elim
        | div => rw [hmkc] at hmk; exact hmk.elim
      | fail e => rw [hsc] at hsub; exact hsub.elim
      | div => rw [hsc] at hsub; exact hsub.elim

/-- **`RefInt.sub`** denotes `iden a − iden b` (it is `add a (neg b)`). -/
private theorem int_sub_eq (a b : RefInt) (ha : IntNorm a) (hb : IntNorm b)
    (hcap : max a.mag.limbs.val.length b.mag.limbs.val.length + 1 ≤ Std.Usize.max) :
    RefInt.sub a b ⦃ r => iden r = iden a - iden b ∧ IntNorm r ⦄ := by
  unfold RefInt.sub
  have hneg := int_neg_eq b hb
  cases hnc : RefInt.impl.neg b with
  | ok nb =>
    rw [hnc] at hneg; simp only [WP.spec_ok] at hneg
    obtain ⟨hnbden, hnbnorm, hnbmag⟩ := hneg
    simp only [bind_tc_ok]
    have hadd := int_add_eq a nb ha hnbnorm (by rw [hnbmag]; exact hcap)
    cases hac : RefInt.add a nb with
    | ok r =>
      rw [hac] at hadd; simp only [WP.spec_ok] at hadd
      obtain ⟨hrden, hrnorm⟩ := hadd
      simp only [WP.spec_ok]
      exact ⟨by rw [hrden, hnbden]; ring, hrnorm⟩
    | fail e => rw [hac] at hadd; exact hadd.elim
    | div => rw [hac] at hadd; exact hadd.elim
  | fail e => rw [hnc] at hneg; exact hneg.elim
  | div => rw [hnc] at hneg; exact hneg.elim

/-- Casting ℕ→ℤ preserves `compare`. -/
private theorem natCast_compare (m n : ℕ) : compare (m : ℤ) (n : ℤ) = compare m n := by
  rcases lt_trichotomy m n with h | h | h
  · rw [compare_lt_iff_lt.mpr (by exact_mod_cast h), compare_lt_iff_lt.mpr h]
  · subst h; rw [Std.ReflOrd.compare_self, Std.ReflOrd.compare_self]
  · rw [compare_gt_iff_gt.mpr (by exact_mod_cast h), compare_gt_iff_gt.mpr h]

/-- Negating both arguments reverses `compare`. -/
private theorem compare_neg_neg (x y : ℤ) : compare (-x) (-y) = compare y x := by
  rcases lt_trichotomy x y with h | h | h
  · rw [compare_gt_iff_gt.mpr (show -y < -x by omega), compare_gt_iff_gt.mpr h]
  · subst h; rw [Std.ReflOrd.compare_self, Std.ReflOrd.compare_self]
  · rw [compare_lt_iff_lt.mpr (show -x < -y by omega), compare_lt_iff_lt.mpr h]

/-- **`RefInt.cmp`** is `compare` on the ℤ denotations (negatives < 0 ≤ nonnegatives; among like
    signs, magnitude order, reversed for negatives). -/
private theorem int_cmp_eq (a b : RefInt) (ha : IntNorm a) (hb : IntNorm b) :
    RefInt.cmp a b = ok (compare (iden a) (iden b)) := by
  unfold RefInt.cmp
  by_cases han : a.neg = true
  · have hia : iden a = -(den a.mag.limbs.val : ℤ) := by simp [iden, han]
    by_cases hbn : b.neg = true
    · have hib : iden b = -(den b.mag.limbs.val : ℤ) := by simp [iden, hbn]
      rw [if_pos han, if_pos hbn, cmp_eq b.mag a.mag hb.1 ha.1, hia, hib, compare_neg_neg,
        natCast_compare]
    · have hbn' : b.neg = false := by simpa using hbn
      have hib : iden b = (den b.mag.limbs.val : ℤ) := by simp [iden, hbn']
      have hane : den a.mag.limbs.val ≠ 0 := fun h => by simp [ha.2 h] at han
      rw [if_pos han, if_neg hbn]
      congr 1; symm; rw [compare_lt_iff_lt, hia, hib]; omega
  · have han' : a.neg = false := by simpa using han
    have hia : iden a = (den a.mag.limbs.val : ℤ) := by simp [iden, han']
    by_cases hbn : b.neg = true
    · have hib : iden b = -(den b.mag.limbs.val : ℤ) := by simp [iden, hbn]
      have hbne : den b.mag.limbs.val ≠ 0 := fun h => by simp [hb.2 h] at hbn
      rw [if_neg han, if_pos hbn]
      congr 1; symm; rw [compare_gt_iff_gt, hia, hib]; omega
    · have hbn' : b.neg = false := by simpa using hbn
      have hib : iden b = (den b.mag.limbs.val : ℤ) := by simp [iden, hbn']
      rw [if_neg han, if_neg hbn, cmp_eq a.mag b.mag ha.1 hb.1, hia, hib, natCast_compare]

/-- **`RefInt.sign`** returns the sign of the ℤ denotation (`Int.sign ∈ {-1,0,1}`). -/
private theorem int_sign_eq (a : RefInt) (ha : IntNorm a) :
    RefInt.sign a ⦃ s => (s.val : ℤ) = Int.sign (iden a) ⦄ := by
  unfold RefInt.sign
  rw [is_zero_eq a.mag ha.1]
  simp only [bind_tc_ok]
  by_cases hz : den a.mag.limbs.val = 0
  · rw [if_pos (by simp [hz])]
    simp only [WP.spec_ok]
    have h0 : iden a = 0 := by simp [iden, hz]
    rw [h0]; decide
  · rw [if_neg (by simp [hz])]
    have hpos : (0 : ℤ) < (den a.mag.limbs.val : ℤ) := by
      have := hz; positivity
    by_cases hn : a.neg = true
    · rw [if_pos hn]
      simp only [WP.spec_ok]
      have hia : iden a = -(den a.mag.limbs.val : ℤ) := by simp [iden, hn]
      rw [hia, Int.sign_eq_neg_one_of_neg (by omega)]; decide
    · rw [if_neg hn]
      simp only [WP.spec_ok]
      have han' : a.neg = false := by simpa using hn
      have hia : iden a = (den a.mag.limbs.val : ℤ) := by simp [iden, han']
      rw [hia, Int.sign_eq_one_of_pos hpos]; decide

/-! ## `RefRat` → ℚ — reduced rationals on top of the `RefInt`/`RefNat` layers

`qden r = iden(r.num) / den(r.den)`. `reduce` divides `±num/den` through by `gcd(num, den)` to lowest
terms with a positive denominator — the constructor every rational op funnels through. -/

/-- Rational denotation of a `RefRat`: signed numerator over the (positive) denominator. -/
def qden (r : RefRat) : ℚ := (iden r.num : ℚ) / (den r.den.limbs.val : ℚ)

/-- Canonical `RefRat`: canonical numerator, normalized denominator, `den > 0`. -/
def RatNorm (r : RefRat) : Prop :=
  IntNorm r.num ∧ Normalized r.den.limbs.val ∧ 0 < den r.den.limbs.val

set_option maxHeartbeats 1000000 in
/-- **`RefRat.reduce`** builds `±num_mag / den_mag` in lowest terms (`den > 0`). -/
private theorem reduce_spec (neg : Bool) (num_mag den_mag : RefNat)
    (hnum : Normalized num_mag.limbs.val) (hden : Normalized den_mag.limbs.val)
    (hdpos : 0 < den den_mag.limbs.val)
    (hnumcap : num_mag.limbs.val.length * 64 ≤ Std.Usize.max)
    (hdencap : den_mag.limbs.val.length * 64 ≤ Std.Usize.max) :
    RefRat.reduce neg num_mag den_mag
      ⦃ r => qden r = (if neg then -(den num_mag.limbs.val : ℚ) else (den num_mag.limbs.val : ℚ))
              / (den den_mag.limbs.val : ℚ) ∧ RatNorm r ⦄ := by
  unfold RefRat.reduce
  rw [is_zero_eq num_mag hnum]
  simp only [bind_tc_ok]
  by_cases hnz : den num_mag.limbs.val = 0
  · rw [if_pos (by simp [hnz])]
    unfold RefInt.zero RefNat.zero
    simp only [bind_tc_ok]
    step
    have hy : (alloc.slice.Slice.into_vec y).val = [1#u64] := by rw [y_post]; rfl
    refine ⟨?_, ⟨fun h => absurd rfl h, fun _ => rfl⟩, ?_, ?_⟩
    · simp [qden, iden, hnz]
    · rw [hy]; intro h; simp
    · rw [hy]; simp [den]
  · rw [if_neg (by simp [hnz])]
    have hnumne : num_mag.limbs.val ≠ [] :=
      fun h => hnz ((den_eq_zero_iff num_mag.limbs.val hnum).mpr h)
    have hnumpos : 0 < den num_mag.limbs.val := Nat.pos_of_ne_zero hnz
    have hnumlen1 : 1 ≤ num_mag.limbs.val.length := List.length_pos_of_ne_nil hnumne
    have hgcd := gcd_eq num_mag den_mag hnum hden hnumcap hdencap
    cases hgc : RefNat.gcd num_mag den_mag with
    | ok g =>
      rw [hgc] at hgcd; simp only [WP.spec_ok] at hgcd
      obtain ⟨hgden, hgnorm⟩ := hgcd
      simp only [bind_tc_ok]
      have hgpos : 0 < den g.limbs.val := by rw [hgden]; exact Nat.gcd_pos_of_pos_left _ hnumpos
      have hgm : den g.limbs.val ∣ den num_mag.limbs.val := by rw [hgden]; exact Nat.gcd_dvd_left _ _
      have hgn : den g.limbs.val ∣ den den_mag.limbs.val := by rw [hgden]; exact Nat.gcd_dvd_right _ _
      have hgle : den g.limbs.val ≤ den num_mag.limbs.val := Nat.le_of_dvd hnumpos hgm
      have hglen : g.limbs.val.length ≤ num_mag.limbs.val.length := by
        by_contra hc; rw [not_le] at hc
        exact absurd (den_lt_of_len_lt num_mag.limbs.val g.limbs.val hgnorm hc) (by omega)
      have hdcap_g : g.limbs.val.length + 1 ≤ Std.Usize.max := by omega
      have hdr1 := divrem_eq num_mag g hnum hgnorm hgpos hdcap_g hnumcap
      cases hdc1 : RefNat.divrem num_mag g with
      | ok qr1 =>
        obtain ⟨nq, rr1⟩ := qr1
        rw [hdc1] at hdr1; simp only [WP.spec_ok] at hdr1
        obtain ⟨hnqden, _, hnqnorm, _⟩ := hdr1
        simp only [bind_tc_ok]
        have hdr2 := divrem_eq den_mag g hden hgnorm hgpos hdcap_g hdencap
        cases hdc2 : RefNat.divrem den_mag g with
        | ok qr2 =>
          obtain ⟨dq, rr2⟩ := qr2
          rw [hdc2] at hdr2; simp only [WP.spec_ok] at hdr2
          obtain ⟨hdqden, _, hdqnorm, _⟩ := hdr2
          simp only [bind_tc_ok]
          show (do let ri ← RefInt.make neg nq; ok (⟨ri, dq⟩ : RefRat))
            ⦃ r => qden r = (if neg then -(den num_mag.limbs.val : ℚ)
                else (den num_mag.limbs.val : ℚ)) / (den den_mag.limbs.val : ℚ) ∧ RatNorm r ⦄
          have hmk := make_spec neg nq hnqnorm
          cases hmkc : RefInt.make neg nq with
          | ok ri =>
            rw [hmkc] at hmk; simp only [WP.spec_ok] at hmk
            obtain ⟨hriden, hrinorm, _⟩ := hmk
            simp only [bind_tc_ok, WP.spec_ok]
            refine ⟨?_, hrinorm, hdqnorm, ?_⟩
            · have hgQ : (den g.limbs.val : ℚ) ≠ 0 := by exact_mod_cast hgpos.ne'
              have hdenQ : (den den_mag.limbs.val : ℚ) ≠ 0 := by exact_mod_cast hdpos.ne'
              have hc1 : ((den num_mag.limbs.val / den g.limbs.val : ℕ) : ℚ)
                  = (den num_mag.limbs.val : ℚ) / den g.limbs.val := Nat.cast_div hgm hgQ
              have hc2 : ((den den_mag.limbs.val / den g.limbs.val : ℕ) : ℚ)
                  = (den den_mag.limbs.val : ℚ) / den g.limbs.val := Nat.cast_div hgn hgQ
              have hnumQ : ((iden ri : ℤ) : ℚ)
                  = (if neg then -(den num_mag.limbs.val : ℚ) else (den num_mag.limbs.val : ℚ))
                    / (den g.limbs.val : ℚ) := by
                rw [hriden, apply_ite (fun x : ℤ => (x : ℚ))]
                simp only [Int.cast_neg, Int.cast_natCast, hnqden, hc1]
                cases neg <;> simp [neg_div]
              have hdenQ2 : ((den dq.limbs.val : ℕ) : ℚ)
                  = (den den_mag.limbs.val : ℚ) / (den g.limbs.val : ℚ) := by rw [hdqden]; exact hc2
              simp only [qden, hnumQ, hdenQ2]
              cases neg <;> field_simp
            · rw [hdqden]; exact Nat.div_pos (Nat.le_of_dvd hdpos hgn) hgpos
          | fail e => rw [hmkc] at hmk; exact hmk.elim
          | div => rw [hmkc] at hmk; exact hmk.elim
        | fail e => rw [hdc2] at hdr2; exact hdr2.elim
        | div => rw [hdc2] at hdr2; exact hdr2.elim
      | fail e => rw [hdc1] at hdr1; exact hdr1.elim
      | div => rw [hdc1] at hdr1; exact hdr1.elim
    | fail e => rw [hgc] at hgcd; exact hgcd.elim
    | div => rw [hgc] at hgcd; exact hgcd.elim

/-! ## `RefBackend`'s `Backend` trait methods — the `RefBackend = ℤ/ℚ` corollary

Each `Backend` method funnels through a proven op refinement above; together they state that the
reference backend computes exact `ℤ`/`ℚ` arithmetic. The `len·64 ≤ usize::MAX` caps are the same
loop-bound side-conditions `bit_len`/`gcd`/`divrem` already carry — always met in practice, made
explicit here. -/

open RefBackend.Insts.LatticeBackendBackendRefIntRefRat

/-- `RefNat::clone` preserves the limb list. -/
private theorem refnat_clone_eq (a : RefNat) :
    RefNat.Insts.CoreCloneClone.clone a ⦃ r => r.limbs.val = a.limbs.val ⦄ := by
  unfold RefNat.Insts.CoreCloneClone.clone
  have hcl : alloc.vec.CloneVec.clone core.clone.CloneU64 a.limbs ⦃ v => a.limbs = v ⦄ :=
    alloc.slice.Slice.to_vec_spec core.clone.CloneU64 a.limbs (by intro x _; rfl)
  cases hcc : alloc.vec.CloneVec.clone core.clone.CloneU64 a.limbs with
  | ok v =>
    rw [hcc] at hcl; simp only [WP.spec_ok] at hcl
    simp only [bind_tc_ok, WP.spec_ok]; rw [← hcl]
  | fail e => rw [hcc] at hcl; exact hcl.elim
  | div => rw [hcc] at hcl; exact hcl.elim

/-- `RefInt::clone` preserves the ℤ denotation and canonical form. -/
private theorem refint_clone_eq (a : RefInt) (ha : IntNorm a) :
    RefInt.Insts.CoreCloneClone.clone a ⦃ r => iden r = iden a ∧ IntNorm r ⦄ := by
  unfold RefInt.Insts.CoreCloneClone.clone
  simp only [core.clone.impls.CloneBool.clone, lift, bind_tc_ok]
  have hcl := refnat_clone_eq a.mag
  cases hcc : RefNat.Insts.CoreCloneClone.clone a.mag with
  | ok rn =>
    rw [hcc] at hcl; simp only [WP.spec_ok] at hcl
    simp only [bind_tc_ok, WP.spec_ok]
    refine ⟨?_, hcl ▸ ha.1, ?_⟩
    · simp only [iden, hcl]
    · rw [hcl]; exact ha.2
  | fail e => rw [hcc] at hcl; exact hcl.elim
  | div => rw [hcc] at hcl; exact hcl.elim

/-- **`RefInt.is_zero`** decides `iden a = 0`. -/
private theorem int_is_zero_eq (a : RefInt) (ha : IntNorm a) :
    RefInt.is_zero a = ok (decide (iden a = 0)) := by
  unfold RefInt.is_zero
  rw [is_zero_eq a.mag ha.1]
  congr 1
  rw [decide_eq_decide]
  simp only [iden]
  cases a.neg <;> simp

/-- **`rat_neg`** negates the rational. -/
private theorem rat_neg_eq (a : RefRat) (ha : RatNorm a) :
    rat_neg a ⦃ r => qden r = -(qden a) ∧ RatNorm r ⦄ := by
  unfold RefBackend.Insts.LatticeBackendBackendRefIntRefRat.rat_neg
  have hneg := int_neg_eq a.num ha.1
  cases hnc : RefInt.impl.neg a.num with
  | ok ri =>
    rw [hnc] at hneg; simp only [WP.spec_ok] at hneg
    obtain ⟨hriden, hrinorm, _⟩ := hneg
    simp only [bind_tc_ok]
    have hcl := refnat_clone_eq a.den
    cases hcc : RefNat.Insts.CoreCloneClone.clone a.den with
    | ok rn =>
      rw [hcc] at hcl; simp only [WP.spec_ok] at hcl
      simp only [bind_tc_ok, WP.spec_ok]
      refine ⟨?_, hrinorm, hcl ▸ ha.2.1, ?_⟩
      · simp only [qden, hriden, hcl, Int.cast_neg, neg_div]
      · rw [hcl]; exact ha.2.2
    | fail e => rw [hcc] at hcl; exact hcl.elim
    | div => rw [hcc] at hcl; exact hcl.elim
  | fail e => rw [hnc] at hneg; exact hneg.elim
  | div => rw [hnc] at hneg; exact hneg.elim

/-- **`rat_numer`** returns the numerator as a `RefInt`. -/
private theorem rat_numer_eq (a : RefRat) (ha : RatNorm a) :
    rat_numer a ⦃ r => iden r = iden a.num ∧ IntNorm r ⦄ := by
  unfold RefBackend.Insts.LatticeBackendBackendRefIntRefRat.rat_numer
  exact refint_clone_eq a.num ha.1

/-- **`rat_denom`** returns the (positive) denominator as a nonnegative `RefInt`. -/
private theorem rat_denom_eq (a : RefRat) (ha : RatNorm a) :
    rat_denom a ⦃ r => iden r = (den a.den.limbs.val : ℤ) ∧ IntNorm r ⦄ := by
  unfold RefBackend.Insts.LatticeBackendBackendRefIntRefRat.rat_denom
  have hcl := refnat_clone_eq a.den
  cases hcc : RefNat.Insts.CoreCloneClone.clone a.den with
  | ok rn =>
    rw [hcc] at hcl; simp only [WP.spec_ok] at hcl
    simp only [bind_tc_ok]
    have hmk := make_spec false rn (hcl ▸ ha.2.1)
    cases hmkc : RefInt.make false rn with
    | ok ri =>
      rw [hmkc] at hmk; simp only [WP.spec_ok] at hmk
      obtain ⟨hriden, hrinorm, _⟩ := hmk
      simp only [WP.spec_ok]
      refine ⟨?_, hrinorm⟩
      rw [hriden, hcl]; simp
    | fail e => rw [hmkc] at hmk; exact hmk.elim
    | div => rw [hmkc] at hmk; exact hmk.elim
  | fail e => rw [hcc] at hcl; exact hcl.elim
  | div => rw [hcc] at hcl; exact hcl.elim

/-- **`rat_is_zero`** decides `qden a = 0`. -/
private theorem rat_is_zero_eq (a : RefRat) (ha : RatNorm a) :
    rat_is_zero a = ok (decide (qden a = 0)) := by
  unfold RefBackend.Insts.LatticeBackendBackendRefIntRefRat.rat_is_zero
  rw [int_is_zero_eq a.num ha.1]
  congr 1
  rw [decide_eq_decide]
  have hd : (den a.den.limbs.val : ℚ) ≠ 0 := by exact_mod_cast ha.2.2.ne'
  rw [qden, div_eq_zero_iff]
  constructor
  · intro h; left; exact_mod_cast h
  · rintro (h | h)
    · exact_mod_cast h
    · exact absurd h hd

/-- **`rat_sign`** returns the sign of the rational (the numerator's sign, since the denominator is
    positive) as an `i8 ∈ {-1, 0, 1}`. -/
private theorem rat_sign_eq (a : RefRat) (ha : RatNorm a) :
    rat_sign a ⦃ s => (s.val : ℤ) = Int.sign (iden a.num) ⦄ := by
  unfold RefBackend.Insts.LatticeBackendBackendRefIntRefRat.rat_sign
  exact int_sign_eq a.num ha.1

/-- Comparing `na/da` and `nb/db` (positive denominators) is comparing the cross-products. -/
private theorem compare_div_div (na nb : ℤ) (da db : ℕ) (hda : 0 < da) (hdb : 0 < db) :
    compare ((na : ℚ) / (da : ℚ)) ((nb : ℚ) / (db : ℚ))
      = compare (na * (db : ℤ)) (nb * (da : ℤ)) := by
  have hdaQ : (0 : ℚ) < (da : ℚ) := by exact_mod_cast hda
  have hdbQ : (0 : ℚ) < (db : ℚ) := by exact_mod_cast hdb
  rcases lt_trichotomy (na * (db : ℤ)) (nb * (da : ℤ)) with h | h | h
  · rw [compare_lt_iff_lt.mpr h, compare_lt_iff_lt.mpr
      (show (na : ℚ) / da < (nb : ℚ) / db by rw [div_lt_div_iff₀ hdaQ hdbQ]; exact_mod_cast h)]
  · have hq : (na : ℚ) / da = (nb : ℚ) / db := by
      rw [div_eq_div_iff hdaQ.ne' hdbQ.ne']; exact_mod_cast h
    rw [hq, h, Std.ReflOrd.compare_self, Std.ReflOrd.compare_self]
  · rw [compare_gt_iff_gt.mpr h, compare_gt_iff_gt.mpr
      (show (nb : ℚ) / db < (na : ℚ) / da by rw [div_lt_div_iff₀ hdbQ hdaQ]; exact_mod_cast h)]

/-- **`rat_cmp`** compares the two rationals (cross-multiplying by the positive denominators). -/
private theorem rat_cmp_eq (a b : RefRat) (ha : RatNorm a) (hb : RatNorm b)
    (hcapl : a.num.mag.limbs.val.length + b.den.limbs.val.length ≤ Std.Usize.max)
    (hcapr : b.num.mag.limbs.val.length + a.den.limbs.val.length ≤ Std.Usize.max) :
    rat_cmp a b = ok (compare (qden a) (qden b)) := by
  unfold RefBackend.Insts.LatticeBackendBackendRefIntRefRat.rat_cmp
  -- `ri = +den(b)`, so `l = a.num · den(b)`
  have hcl1 := refnat_clone_eq b.den
  cases hcc1 : RefNat.Insts.CoreCloneClone.clone b.den with
  | ok rn =>
    rw [hcc1] at hcl1; simp only [WP.spec_ok] at hcl1
    simp only [bind_tc_ok]
    have hmk1 := make_spec false rn (hcl1 ▸ hb.2.1)
    cases hmkc1 : RefInt.make false rn with
    | ok ri =>
      rw [hmkc1] at hmk1; simp only [WP.spec_ok] at hmk1
      obtain ⟨hri1den, hri1norm, hri1mag⟩ := hmk1
      have hri1 : iden ri = (den b.den.limbs.val : ℤ) := by rw [hri1den, hcl1]; simp
      have hri1len : ri.mag.limbs.val.length = b.den.limbs.val.length := by rw [hri1mag, hcl1]
      simp only [bind_tc_ok]
      have hmul1 := int_mul_eq a.num ri (by rw [hri1len]; exact hcapl)
      cases hmc1 : RefInt.mul a.num ri with
      | ok l =>
        rw [hmc1] at hmul1; simp only [WP.spec_ok] at hmul1
        obtain ⟨hlden, hlnorm⟩ := hmul1
        simp only [bind_tc_ok]
        -- `ri1 = +den(a)`, so `r = b.num · den(a)`
        have hcl2 := refnat_clone_eq a.den
        cases hcc2 : RefNat.Insts.CoreCloneClone.clone a.den with
        | ok rn1 =>
          rw [hcc2] at hcl2; simp only [WP.spec_ok] at hcl2
          simp only [bind_tc_ok]
          have hmk2 := make_spec false rn1 (hcl2 ▸ ha.2.1)
          cases hmkc2 : RefInt.make false rn1 with
          | ok ri1 =>
            rw [hmkc2] at hmk2; simp only [WP.spec_ok] at hmk2
            obtain ⟨hri2den, hri2norm, hri2mag⟩ := hmk2
            have hri2 : iden ri1 = (den a.den.limbs.val : ℤ) := by rw [hri2den, hcl2]; simp
            have hri2len : ri1.mag.limbs.val.length = a.den.limbs.val.length := by rw [hri2mag, hcl2]
            simp only [bind_tc_ok]
            have hmul2 := int_mul_eq b.num ri1 (by rw [hri2len]; exact hcapr)
            cases hmc2 : RefInt.mul b.num ri1 with
            | ok r =>
              rw [hmc2] at hmul2; simp only [WP.spec_ok] at hmul2
              obtain ⟨hrden, hrnorm⟩ := hmul2
              simp only [bind_tc_ok]
              rw [int_cmp_eq l r hlnorm hrnorm, hlden, hrden, hri1, hri2]
              simp only [qden]
              rw [compare_div_div _ _ _ _ ha.2.2 hb.2.2]
            | fail e => rw [hmc2] at hmul2; exact hmul2.elim
            | div => rw [hmc2] at hmul2; exact hmul2.elim
          | fail e => rw [hmkc2] at hmk2; exact hmk2.elim
          | div => rw [hmkc2] at hmk2; exact hmk2.elim
        | fail e => rw [hcc2] at hcl2; exact hcl2.elim
        | div => rw [hcc2] at hcl2; exact hcl2.elim
      | fail e => rw [hmc1] at hmul1; exact hmul1.elim
      | div => rw [hmc1] at hmul1; exact hmul1.elim
    | fail e => rw [hmkc1] at hmk1; exact hmk1.elim
    | div => rw [hmkc1] at hmk1; exact hmk1.elim
  | fail e => rw [hcc1] at hcl1; exact hcl1.elim
  | div => rw [hcc1] at hcl1; exact hcl1.elim

/-- **`rat_from_ints`** builds `num / dn` in lowest terms (`dn ≠ 0`). -/
private theorem rat_from_ints_eq (num dn : RefInt) (hnum : IntNorm num) (hdn : IntNorm dn)
    (hdn0 : den dn.mag.limbs.val ≠ 0)
    (hnumcap : num.mag.limbs.val.length * 64 ≤ Std.Usize.max)
    (hdncap : dn.mag.limbs.val.length * 64 ≤ Std.Usize.max) :
    rat_from_ints num dn
      ⦃ r => qden r = (iden num : ℚ) / (iden dn : ℚ) ∧ RatNorm r ⦄ := by
  unfold RefBackend.Insts.LatticeBackendBackendRefIntRefRat.rat_from_ints
  have hidenz : ¬ (iden dn = 0) := by simp only [iden]; cases dn.neg <;> simp [hdn0]
  have hb : decide (iden dn = 0) = false := by simp only [decide_eq_false_iff_not]; exact hidenz
  rw [int_is_zero_eq dn hdn, hb]
  -- `dn ≠ 0`, so the assertion passes definitionally and the prefix reduces to `reduce`
  show RefRat.reduce (num.neg != dn.neg) num.mag dn.mag
    ⦃ r => qden r = (iden num : ℚ) / (iden dn : ℚ) ∧ RatNorm r ⦄
  have hdpos : 0 < den dn.mag.limbs.val := Nat.pos_of_ne_zero hdn0
  have hred := reduce_spec (num.neg != dn.neg) num.mag dn.mag hnum.1 hdn.1 hdpos hnumcap hdncap
  cases hrc : RefRat.reduce (num.neg != dn.neg) num.mag dn.mag with
  | ok r =>
    rw [hrc] at hred; simp only [WP.spec_ok] at hred
    obtain ⟨hrval, hrnorm⟩ := hred
    simp only [WP.spec_ok]
    refine ⟨?_, hrnorm⟩
    rw [hrval]
    have hidenum : (iden num : ℚ)
        = if num.neg then -(den num.mag.limbs.val : ℚ) else (den num.mag.limbs.val : ℚ) := by
      simp only [iden]; cases num.neg <;> simp
    have hideden : (iden dn : ℚ)
        = if dn.neg then -(den dn.mag.limbs.val : ℚ) else (den dn.mag.limbs.val : ℚ) := by
      simp only [iden]; cases dn.neg <;> simp
    rw [hidenum, hideden]
    cases num.neg <;> cases dn.neg <;> simp [neg_div, div_neg]
  | fail e => rw [hrc] at hred; exact hred.elim
  | div => rw [hrc] at hred; exact hred.elim

/-! ### The rat *arithmetic* methods — each funnels a product through `reduce`.

`reduce` needs `len·64 ≤ usize::MAX` on its numerator/denominator; the length of a product is bounded
by the sum of the factor lengths (`den_mul_len_le`), which turns that into a cap on the inputs. -/

/-- A normalized product `den r = den x · den y` has length `≤ x.length + y.length`: its value is
    `< 2^(64(|x|+|y|))`, and a normalized length-`n` list denotes `≥ 2^(64(n−1))`. -/
private theorem den_mul_len_le (r x y : List Std.U64) (hr : Normalized r)
    (hD : den r = den x * den y) : r.length ≤ x.length + y.length := by
  rcases eq_or_ne r [] with h | h
  · simp [h]
  · have hrpos : 1 ≤ r.length := List.length_pos_of_ne_nil h
    have hlow := den_lower r h hr
    have hub : den r < 2 ^ (64 * (x.length + y.length)) := by
      rw [hD]
      calc den x * den y ≤ den x * 2 ^ (64 * y.length) :=
              Nat.mul_le_mul (le_refl _) (le_of_lt (den_lt y))
        _ < 2 ^ (64 * x.length) * 2 ^ (64 * y.length) :=
            mul_lt_mul_of_pos_right (den_lt x) (pow_pos (by norm_num) _)
        _ = 2 ^ (64 * (x.length + y.length)) := by rw [← pow_add]; congr 1; ring
    have hlt : 64 * (r.length - 1) < 64 * (x.length + y.length) :=
      (Nat.pow_lt_pow_iff_right (by norm_num)).mp (lt_of_le_of_lt hlow hub)
    omega

/-- `|iden a| = den(a.mag)` — the magnitude is the natural-number absolute value. -/
private theorem iden_natAbs (a : RefInt) : (iden a).natAbs = den a.mag.limbs.val := by
  simp only [iden]; cases a.neg <;> simp

/-- **`rat_mul`** multiplies the two rationals. -/
private theorem rat_mul_eq (a b : RefRat) (ha : RatNorm a) (hb : RatNorm b)
    (hncap : (a.num.mag.limbs.val.length + b.num.mag.limbs.val.length) * 64 ≤ Std.Usize.max)
    (hdcap : (a.den.limbs.val.length + b.den.limbs.val.length) * 64 ≤ Std.Usize.max) :
    rat_mul a b ⦃ r => qden r = qden a * qden b ∧ RatNorm r ⦄ := by
  unfold RefBackend.Insts.LatticeBackendBackendRefIntRefRat.rat_mul
  have hmul := int_mul_eq a.num b.num (by omega)
  cases hmc : RefInt.mul a.num b.num with
  | ok n =>
    rw [hmc] at hmul; simp only [WP.spec_ok] at hmul
    obtain ⟨hniden, hnnorm⟩ := hmul
    simp only [bind_tc_ok]
    have hmuld := mul_eq a.den b.den (by omega)
    cases hmdc : RefNat.mul a.den b.den with
    | ok rn =>
      rw [hmdc] at hmuld; simp only [WP.spec_ok] at hmuld
      obtain ⟨hrnden, hrnnorm⟩ := hmuld
      simp only [bind_tc_ok]
      have hnmag : den n.mag.limbs.val
          = den a.num.mag.limbs.val * den b.num.mag.limbs.val := by
        rw [← iden_natAbs n, hniden, Int.natAbs_mul, iden_natAbs, iden_natAbs]
      have hnlen := den_mul_len_le _ _ _ hnnorm.1 hnmag
      have hnmagcap : n.mag.limbs.val.length * 64 ≤ Std.Usize.max := by omega
      have hrnlen := den_mul_len_le _ _ _ hrnnorm hrnden
      have hrncap : rn.limbs.val.length * 64 ≤ Std.Usize.max := by omega
      have hrnpos : 0 < den rn.limbs.val := by rw [hrnden]; exact Nat.mul_pos ha.2.2 hb.2.2
      have hred := reduce_spec n.neg n.mag rn hnnorm.1 hrnnorm hrnpos hnmagcap hrncap
      cases hrc : RefRat.reduce n.neg n.mag rn with
      | ok r =>
        rw [hrc] at hred; simp only [WP.spec_ok] at hred
        obtain ⟨hrval, hrnorm⟩ := hred
        simp only [WP.spec_ok]
        refine ⟨?_, hrnorm⟩
        rw [hrval]
        have hncast : (if n.neg then -(den n.mag.limbs.val : ℚ) else (den n.mag.limbs.val : ℚ))
            = (iden n : ℚ) := by simp only [iden]; cases n.neg <;> simp
        rw [hncast, hniden, hrnden]
        simp only [qden, Int.cast_mul, Nat.cast_mul, div_mul_div_comm]
      | fail e => rw [hrc] at hred; exact hred.elim
      | div => rw [hrc] at hred; exact hred.elim
    | fail e => rw [hmdc] at hmuld; exact hmuld.elim
    | div => rw [hmdc] at hmuld; exact hmuld.elim
  | fail e => rw [hmc] at hmul; exact hmul.elim
  | div => rw [hmc] at hmul; exact hmul.elim

/-- **`rat_div`** divides the two rationals (`b ≠ 0`). -/
private theorem rat_div_eq (a b : RefRat) (ha : RatNorm a) (hb : RatNorm b)
    (hb0 : den b.num.mag.limbs.val ≠ 0)
    (hncap : (a.num.mag.limbs.val.length + b.den.limbs.val.length) * 64 ≤ Std.Usize.max)
    (hdcap : (a.den.limbs.val.length + b.num.mag.limbs.val.length) * 64 ≤ Std.Usize.max) :
    rat_div a b ⦃ r => qden r = qden a / qden b ∧ RatNorm r ⦄ := by
  unfold RefBackend.Insts.LatticeBackendBackendRefIntRefRat.rat_div
  have hidenz : ¬ (iden b.num = 0) := by simp only [iden]; cases b.num.neg <;> simp [hb0]
  have hbz : decide (iden b.num = 0) = false := by simp only [decide_eq_false_iff_not]; exact hidenz
  rw [int_is_zero_eq b.num hb.1, hbz]
  show (do
      let rn ← RefNat.Insts.CoreCloneClone.clone b.den
      let ri ← RefInt.make false rn
      let n ← RefInt.mul a.num ri
      let rn1 ← RefNat.mul a.den b.num.mag
      RefRat.reduce (n.neg != b.num.neg) n.mag rn1)
    ⦃ r => qden r = qden a / qden b ∧ RatNorm r ⦄
  have hcl := refnat_clone_eq b.den
  cases hcc : RefNat.Insts.CoreCloneClone.clone b.den with
  | ok rn =>
    rw [hcc] at hcl; simp only [WP.spec_ok] at hcl
    simp only [bind_tc_ok]
    have hmk := make_spec false rn (hcl ▸ hb.2.1)
    cases hmkc : RefInt.make false rn with
    | ok ri =>
      rw [hmkc] at hmk; simp only [WP.spec_ok] at hmk
      obtain ⟨hriden, hrinorm, hrimag⟩ := hmk
      have hri : iden ri = (den b.den.limbs.val : ℤ) := by rw [hriden, hcl]; simp
      have hrilen : ri.mag.limbs.val.length = b.den.limbs.val.length := by rw [hrimag, hcl]
      simp only [bind_tc_ok]
      have hmul := int_mul_eq a.num ri (by rw [hrilen]; omega)
      cases hmc : RefInt.mul a.num ri with
      | ok n =>
        rw [hmc] at hmul; simp only [WP.spec_ok] at hmul
        obtain ⟨hniden, hnnorm⟩ := hmul
        simp only [bind_tc_ok]
        have hmuld := mul_eq a.den b.num.mag (by omega)
        cases hmdc : RefNat.mul a.den b.num.mag with
        | ok rn1 =>
          rw [hmdc] at hmuld; simp only [WP.spec_ok] at hmuld
          obtain ⟨hrn1den, hrn1norm⟩ := hmuld
          simp only [bind_tc_ok]
          have hnmag : den n.mag.limbs.val = den a.num.mag.limbs.val * den b.den.limbs.val := by
            rw [← iden_natAbs n, hniden, Int.natAbs_mul, iden_natAbs]
            rw [show (iden ri).natAbs = den b.den.limbs.val by rw [hri]; simp]
          have hnlen := den_mul_len_le _ _ _ hnnorm.1 hnmag
          have hnmagcap : n.mag.limbs.val.length * 64 ≤ Std.Usize.max := by omega
          have hrn1len := den_mul_len_le _ _ _ hrn1norm hrn1den
          have hrn1cap : rn1.limbs.val.length * 64 ≤ Std.Usize.max := by omega
          have hrn1pos : 0 < den rn1.limbs.val := by
            rw [hrn1den]; exact Nat.mul_pos ha.2.2 (Nat.pos_of_ne_zero hb0)
          have hred := reduce_spec (n.neg != b.num.neg) n.mag rn1 hnnorm.1 hrn1norm hrn1pos
            hnmagcap hrn1cap
          cases hrc : RefRat.reduce (n.neg != b.num.neg) n.mag rn1 with
          | ok r =>
            rw [hrc] at hred; simp only [WP.spec_ok] at hred
            obtain ⟨hrval, hrnorm⟩ := hred
            simp only [WP.spec_ok]
            refine ⟨?_, hrnorm⟩
            rw [hrval]
            have hncast : (if n.neg then -(den n.mag.limbs.val : ℚ) else (den n.mag.limbs.val : ℚ))
                = (iden n : ℚ) := by simp only [iden]; cases n.neg <;> simp
            have hsplit : (if (n.neg != b.num.neg) then -(den n.mag.limbs.val : ℚ)
                  else (den n.mag.limbs.val : ℚ))
                = (iden n : ℚ) * (if b.num.neg then (-1 : ℚ) else 1) := by
              rw [← hncast]; cases n.neg <;> cases b.num.neg <;> simp
            have hnq : (iden n : ℚ) = (iden a.num : ℚ) * (den b.den.limbs.val : ℚ) := by
              rw [hniden, hri]; push_cast; ring
            have hbnum : (iden b.num : ℚ)
                = (if b.num.neg then (-1 : ℚ) else 1) * (den b.num.mag.limbs.val : ℚ) := by
              simp only [iden]; cases b.num.neg <;> simp
            have hsgn : (if b.num.neg then (-1 : ℚ) else 1) ≠ 0 := by cases b.num.neg <;> simp
            have hdb : (den b.num.mag.limbs.val : ℚ) ≠ 0 := by exact_mod_cast hb0
            have hda : (den a.den.limbs.val : ℚ) ≠ 0 := by exact_mod_cast ha.2.2.ne'
            have hdbd : (den b.den.limbs.val : ℚ) ≠ 0 := by exact_mod_cast hb.2.2.ne'
            rw [hsplit, hrn1den, hnq]
            simp only [qden, hbnum, Nat.cast_mul]
            field_simp
            cases b.num.neg <;> simp
          | fail e => rw [hrc] at hred; exact hred.elim
          | div => rw [hrc] at hred; exact hred.elim
        | fail e => rw [hmdc] at hmuld; exact hmuld.elim
        | div => rw [hmdc] at hmuld; exact hmuld.elim
      | fail e => rw [hmc] at hmul; exact hmul.elim
      | div => rw [hmc] at hmul; exact hmul.elim
    | fail e => rw [hmkc] at hmk; exact hmk.elim
    | div => rw [hmkc] at hmk; exact hmk.elim
  | fail e => rw [hcc] at hcl; exact hcl.elim
  | div => rw [hcc] at hcl; exact hcl.elim

/-- A normalized `den r ≤ den x + den y` has length `≤ max x.length y.length + 1` (a sum of two
    values each `< 2^(64 M)` is `< 2^(64(M+1))`, `M = max`). -/
private theorem den_add_len_le (r x y : List Std.U64) (hr : Normalized r)
    (hD : den r ≤ den x + den y) : r.length ≤ max x.length y.length + 1 := by
  rcases eq_or_ne r [] with h | h
  · simp [h]
  · have hrpos : 1 ≤ r.length := List.length_pos_of_ne_nil h
    have hlow := den_lower r h hr
    have hxM : den x < 2 ^ (64 * max x.length y.length) :=
      lt_of_lt_of_le (den_lt x) (Nat.pow_le_pow_right (by norm_num) (by omega))
    have hyM : den y < 2 ^ (64 * max x.length y.length) :=
      lt_of_lt_of_le (den_lt y) (Nat.pow_le_pow_right (by norm_num) (by omega))
    have hub : den r < 2 ^ (64 * (max x.length y.length + 1)) := by
      have h2 : 2 ^ (64 * max x.length y.length) + 2 ^ (64 * max x.length y.length)
          ≤ 2 ^ (64 * (max x.length y.length + 1)) := by rw [Nat.mul_succ, pow_add]; ring_nf; omega
      omega
    have hlt : 64 * (r.length - 1) < 64 * (max x.length y.length + 1) :=
      (Nat.pow_lt_pow_iff_right (by norm_num)).mp (lt_of_le_of_lt hlow hub)
    omega

/-- **`rat_add`** adds the two rationals. -/
private theorem rat_add_eq (a b : RefRat) (ha : RatNorm a) (hb : RatNorm b)
    (hcap : (a.num.mag.limbs.val.length + b.den.limbs.val.length
        + b.num.mag.limbs.val.length + a.den.limbs.val.length + 1) * 64 ≤ Std.Usize.max)
    (hdcap : (a.den.limbs.val.length + b.den.limbs.val.length) * 64 ≤ Std.Usize.max) :
    rat_add a b ⦃ r => qden r = qden a + qden b ∧ RatNorm r ⦄ := by
  unfold RefBackend.Insts.LatticeBackendBackendRefIntRefRat.rat_add
  have hcl := refnat_clone_eq b.den
  cases hcc : RefNat.Insts.CoreCloneClone.clone b.den with
  | ok rn =>
    rw [hcc] at hcl; simp only [WP.spec_ok] at hcl
    simp only [bind_tc_ok]
    have hmk := make_spec false rn (hcl ▸ hb.2.1)
    cases hmkc : RefInt.make false rn with
    | ok ri =>
      rw [hmkc] at hmk; simp only [WP.spec_ok] at hmk
      obtain ⟨hriden, hrinorm, hrimag⟩ := hmk
      have hri : iden ri = (den b.den.limbs.val : ℤ) := by rw [hriden, hcl]; simp
      have hrilen : ri.mag.limbs.val.length = b.den.limbs.val.length := by rw [hrimag, hcl]
      simp only [bind_tc_ok]
      have hmul1 := int_mul_eq a.num ri (by rw [hrilen]; omega)
      cases hmc1 : RefInt.mul a.num ri with
      | ok n1 =>
        rw [hmc1] at hmul1; simp only [WP.spec_ok] at hmul1
        obtain ⟨hn1iden, hn1norm⟩ := hmul1
        have hn1mag : den n1.mag.limbs.val = den a.num.mag.limbs.val * den b.den.limbs.val := by
          rw [← iden_natAbs n1, hn1iden, Int.natAbs_mul, iden_natAbs]
          rw [show (iden ri).natAbs = den b.den.limbs.val by rw [hri]; simp]
        have hn1len := den_mul_len_le _ _ _ hn1norm.1 hn1mag
        simp only [bind_tc_ok]
        have hcl2 := refnat_clone_eq a.den
        cases hcc2 : RefNat.Insts.CoreCloneClone.clone a.den with
        | ok rn1 =>
          rw [hcc2] at hcl2; simp only [WP.spec_ok] at hcl2
          simp only [bind_tc_ok]
          have hmk2 := make_spec false rn1 (hcl2 ▸ ha.2.1)
          cases hmkc2 : RefInt.make false rn1 with
          | ok ri1 =>
            rw [hmkc2] at hmk2; simp only [WP.spec_ok] at hmk2
            obtain ⟨hri1den, hri1norm, hri1mag⟩ := hmk2
            have hri1 : iden ri1 = (den a.den.limbs.val : ℤ) := by rw [hri1den, hcl2]; simp
            have hri1len : ri1.mag.limbs.val.length = a.den.limbs.val.length := by rw [hri1mag, hcl2]
            simp only [bind_tc_ok]
            have hmul2 := int_mul_eq b.num ri1 (by rw [hri1len]; omega)
            cases hmc2 : RefInt.mul b.num ri1 with
            | ok n2 =>
              rw [hmc2] at hmul2; simp only [WP.spec_ok] at hmul2
              obtain ⟨hn2iden, hn2norm⟩ := hmul2
              have hn2mag : den n2.mag.limbs.val = den b.num.mag.limbs.val * den a.den.limbs.val := by
                rw [← iden_natAbs n2, hn2iden, Int.natAbs_mul, iden_natAbs]
                rw [show (iden ri1).natAbs = den a.den.limbs.val by rw [hri1]; simp]
              have hn2len := den_mul_len_le _ _ _ hn2norm.1 hn2mag
              simp only [bind_tc_ok]
              have hadd := int_add_eq n1 n2 hn1norm hn2norm (by omega)
              cases hac : RefInt.add n1 n2 with
              | ok n =>
                rw [hac] at hadd; simp only [WP.spec_ok] at hadd
                obtain ⟨hniden, hnnorm⟩ := hadd
                have hnmag : den n.mag.limbs.val ≤ den n1.mag.limbs.val + den n2.mag.limbs.val := by
                  rw [← iden_natAbs n, hniden]
                  calc (iden n1 + iden n2).natAbs
                      ≤ (iden n1).natAbs + (iden n2).natAbs := Int.natAbs_add_le _ _
                    _ = den n1.mag.limbs.val + den n2.mag.limbs.val := by rw [iden_natAbs, iden_natAbs]
                have hnlen := den_add_len_le _ _ _ hnnorm.1 hnmag
                have hnmagcap : n.mag.limbs.val.length * 64 ≤ Std.Usize.max := by omega
                simp only [bind_tc_ok]
                have hmuld := mul_eq a.den b.den (by omega)
                cases hmdc : RefNat.mul a.den b.den with
                | ok rn2 =>
                  rw [hmdc] at hmuld; simp only [WP.spec_ok] at hmuld
                  obtain ⟨hrn2den, hrn2norm⟩ := hmuld
                  have hrn2len := den_mul_len_le _ _ _ hrn2norm hrn2den
                  have hrn2cap : rn2.limbs.val.length * 64 ≤ Std.Usize.max := by omega
                  have hrn2pos : 0 < den rn2.limbs.val := by
                    rw [hrn2den]; exact Nat.mul_pos ha.2.2 hb.2.2
                  simp only [bind_tc_ok]
                  have hred := reduce_spec n.neg n.mag rn2 hnnorm.1 hrn2norm hrn2pos hnmagcap hrn2cap
                  cases hrc : RefRat.reduce n.neg n.mag rn2 with
                  | ok r =>
                    rw [hrc] at hred; simp only [WP.spec_ok] at hred
                    obtain ⟨hrval, hrnorm⟩ := hred
                    simp only [WP.spec_ok]
                    refine ⟨?_, hrnorm⟩
                    rw [hrval]
                    have hncast : (if n.neg then -(den n.mag.limbs.val : ℚ)
                        else (den n.mag.limbs.val : ℚ)) = (iden n : ℚ) := by
                      simp only [iden]; cases n.neg <;> simp
                    have hn1v : (iden n1 : ℚ) = (iden a.num : ℚ) * (den b.den.limbs.val : ℚ) := by
                      rw [hn1iden, hri]; push_cast; ring
                    have hn2v : (iden n2 : ℚ) = (iden b.num : ℚ) * (den a.den.limbs.val : ℚ) := by
                      rw [hn2iden, hri1]; push_cast; ring
                    have hnval : (iden n : ℚ)
                        = (iden a.num : ℚ) * (den b.den.limbs.val : ℚ)
                          + (iden b.num : ℚ) * (den a.den.limbs.val : ℚ) := by
                      rw [hniden, Int.cast_add, hn1v, hn2v]
                    have hrn2v : (den rn2.limbs.val : ℚ)
                        = (den a.den.limbs.val : ℚ) * (den b.den.limbs.val : ℚ) := by
                      rw [hrn2den]; push_cast; ring
                    have hda : (den a.den.limbs.val : ℚ) ≠ 0 := by exact_mod_cast ha.2.2.ne'
                    have hdbd : (den b.den.limbs.val : ℚ) ≠ 0 := by exact_mod_cast hb.2.2.ne'
                    rw [hncast, hnval, hrn2v]
                    simp only [qden]
                    field_simp
                  | fail e => rw [hrc] at hred; exact hred.elim
                  | div => rw [hrc] at hred; exact hred.elim
                | fail e => rw [hmdc] at hmuld; exact hmuld.elim
                | div => rw [hmdc] at hmuld; exact hmuld.elim
              | fail e => rw [hac] at hadd; exact hadd.elim
              | div => rw [hac] at hadd; exact hadd.elim
            | fail e => rw [hmc2] at hmul2; exact hmul2.elim
            | div => rw [hmc2] at hmul2; exact hmul2.elim
          | fail e => rw [hmkc2] at hmk2; exact hmk2.elim
          | div => rw [hmkc2] at hmk2; exact hmk2.elim
        | fail e => rw [hcc2] at hcl2; exact hcl2.elim
        | div => rw [hcc2] at hcl2; exact hcl2.elim
      | fail e => rw [hmc1] at hmul1; exact hmul1.elim
      | div => rw [hmc1] at hmul1; exact hmul1.elim
    | fail e => rw [hmkc] at hmk; exact hmk.elim
    | div => rw [hmkc] at hmk; exact hmk.elim
  | fail e => rw [hcc] at hcl; exact hcl.elim
  | div => rw [hcc] at hcl; exact hcl.elim

set_option maxHeartbeats 400000 in
/-- **`rat_sub`** subtracts the two rationals — it is `rat_add a (−b)`. -/
private theorem rat_sub_eq (a b : RefRat) (ha : RatNorm a) (hb : RatNorm b)
    (hcap : (a.num.mag.limbs.val.length + b.den.limbs.val.length
        + b.num.mag.limbs.val.length + a.den.limbs.val.length + 1) * 64 ≤ Std.Usize.max)
    (hdcap : (a.den.limbs.val.length + b.den.limbs.val.length) * 64 ≤ Std.Usize.max) :
    rat_sub a b ⦃ r => qden r = qden a - qden b ∧ RatNorm r ⦄ := by
  unfold RefBackend.Insts.LatticeBackendBackendRefIntRefRat.rat_sub
  have hneg := int_neg_eq b.num hb.1
  cases hnc : RefInt.impl.neg b.num with
  | ok ri =>
    rw [hnc] at hneg; simp only [WP.spec_ok] at hneg
    obtain ⟨hriden, hrinorm, hrimag⟩ := hneg
    simp only [bind_tc_ok]
    have hcl := refnat_clone_eq b.den
    cases hcc : RefNat.Insts.CoreCloneClone.clone b.den with
    | ok rn =>
      rw [hcc] at hcl; simp only [WP.spec_ok] at hcl
      simp only [bind_tc_ok]
      -- keep the negated rational opaque (`c`) so nothing reduces the giant `rat_add` body
      set c : RefRat := ⟨ri, rn⟩ with hc
      have hnorm2 : RatNorm c := by
        rw [hc]; exact ⟨hrinorm, hcl ▸ hb.2.1, by rw [hcl]; exact hb.2.2⟩
      have hd2len : c.den.limbs.val.length = b.den.limbs.val.length := by
        rw [hc]; exact congrArg List.length hcl
      have hn2len : c.num.mag.limbs.val.length = b.num.mag.limbs.val.length := by
        rw [hc]; exact congrArg List.length hrimag
      have hqb : qden c = -(qden b) := by
        rw [hc]; show (iden ri : ℚ) / (den rn.limbs.val : ℚ) = -(qden b)
        simp only [qden, hriden, hcl, Int.cast_neg, neg_div]
      have hcap2 : (a.num.mag.limbs.val.length + c.den.limbs.val.length
          + c.num.mag.limbs.val.length + a.den.limbs.val.length + 1) * 64 ≤ Std.Usize.max := by
        rw [hd2len, hn2len]; exact hcap
      have hdcap2 : (a.den.limbs.val.length + c.den.limbs.val.length) * 64 ≤ Std.Usize.max := by
        rw [hd2len]; exact hdcap
      have hadd := rat_add_eq a c ha hnorm2 hcap2 hdcap2
      cases hac : rat_add a c with
      | ok r =>
        rw [hac] at hadd; simp only [WP.spec_ok] at hadd
        obtain ⟨hrval, hrnorm⟩ := hadd
        simp only [WP.spec_ok]
        exact ⟨by rw [hrval, hqb]; ring, hrnorm⟩
      | fail e => rw [hac] at hadd; exact hadd.elim
      | div => rw [hac] at hadd; exact hadd.elim
    | fail e => rw [hcc] at hcl; exact hcl.elim
    | div => rw [hcc] at hcl; exact hcl.elim
  | fail e => rw [hnc] at hneg; exact hneg.elim
  | div => rw [hnc] at hneg; exact hneg.elim

/-! ### The `int_*` `Backend` methods — thin wrappers over the proven `RefInt` ops. -/

/-- **`int_zero`** denotes `0`. -/
private theorem int_zero_backend_eq :
    (int_zero : Result RefInt) ⦃ r => iden r = 0 ∧ IntNorm r ⦄ := by
  unfold RefBackend.Insts.LatticeBackendBackendRefIntRefRat.int_zero; exact int_zero_eq

/-- **`int_add`** denotes `iden a + iden b`. -/
private theorem int_add_backend_eq (a b : RefInt) (ha : IntNorm a) (hb : IntNorm b)
    (hcap : max a.mag.limbs.val.length b.mag.limbs.val.length + 1 ≤ Std.Usize.max) :
    int_add a b ⦃ r => iden r = iden a + iden b ∧ IntNorm r ⦄ := by
  unfold RefBackend.Insts.LatticeBackendBackendRefIntRefRat.int_add; exact int_add_eq a b ha hb hcap

/-- **`int_sub`** denotes `iden a − iden b`. -/
private theorem int_sub_backend_eq (a b : RefInt) (ha : IntNorm a) (hb : IntNorm b)
    (hcap : max a.mag.limbs.val.length b.mag.limbs.val.length + 1 ≤ Std.Usize.max) :
    int_sub a b ⦃ r => iden r = iden a - iden b ∧ IntNorm r ⦄ := by
  unfold RefBackend.Insts.LatticeBackendBackendRefIntRefRat.int_sub; exact int_sub_eq a b ha hb hcap

/-- **`int_mul`** denotes `iden a · iden b`. -/
private theorem int_mul_backend_eq (a b : RefInt)
    (hcap : a.mag.limbs.val.length + b.mag.limbs.val.length ≤ Std.Usize.max) :
    int_mul a b ⦃ r => iden r = iden a * iden b ∧ IntNorm r ⦄ := by
  unfold RefBackend.Insts.LatticeBackendBackendRefIntRefRat.int_mul; exact int_mul_eq a b hcap

/-- **`int_neg`** denotes `-iden a`. -/
private theorem int_neg_backend_eq (a : RefInt) (ha : IntNorm a) :
    int_neg a ⦃ r => iden r = -(iden a) ∧ IntNorm r ∧ r.mag.limbs.val = a.mag.limbs.val ⦄ := by
  unfold RefBackend.Insts.LatticeBackendBackendRefIntRefRat.int_neg; exact int_neg_eq a ha

/-- **`int_cmp`** is `compare` on the ℤ denotations. -/
private theorem int_cmp_backend_eq (a b : RefInt) (ha : IntNorm a) (hb : IntNorm b) :
    int_cmp a b = ok (compare (iden a) (iden b)) := by
  unfold RefBackend.Insts.LatticeBackendBackendRefIntRefRat.int_cmp; exact int_cmp_eq a b ha hb

/-- **`int_sign`** returns the sign of the ℤ denotation. -/
private theorem int_sign_backend_eq (a : RefInt) (ha : IntNorm a) :
    int_sign a ⦃ s => (s.val : ℤ) = Int.sign (iden a) ⦄ := by
  unfold RefBackend.Insts.LatticeBackendBackendRefIntRefRat.int_sign; exact int_sign_eq a ha

/-- **`int_is_zero`** decides `iden a = 0`. -/
private theorem int_is_zero_backend_eq (a : RefInt) (ha : IntNorm a) :
    int_is_zero a = ok (decide (iden a = 0)) := by
  unfold RefBackend.Insts.LatticeBackendBackendRefIntRefRat.int_is_zero; exact int_is_zero_eq a ha

/-- **`int_gcd`** is `gcd(|a|, |b|)` as a nonnegative `RefInt`. -/
private theorem int_gcd_eq (a b : RefInt) (ha : IntNorm a) (hb : IntNorm b)
    (hacap : a.mag.limbs.val.length * 64 ≤ Std.Usize.max)
    (hbcap : b.mag.limbs.val.length * 64 ≤ Std.Usize.max) :
    int_gcd a b
      ⦃ r => iden r = (Nat.gcd (den a.mag.limbs.val) (den b.mag.limbs.val) : ℤ) ∧ IntNorm r ⦄ := by
  unfold RefBackend.Insts.LatticeBackendBackendRefIntRefRat.int_gcd
  have hg := gcd_eq a.mag b.mag ha.1 hb.1 hacap hbcap
  cases hgc : RefNat.gcd a.mag b.mag with
  | ok g =>
    rw [hgc] at hg; simp only [WP.spec_ok] at hg
    obtain ⟨hgden, hgnorm⟩ := hg
    simp only [bind_tc_ok]
    have hmk := make_spec false g hgnorm
    cases hmkc : RefInt.make false g with
    | ok r =>
      rw [hmkc] at hmk; simp only [WP.spec_ok] at hmk
      obtain ⟨hrden, hrnorm, _⟩ := hmk
      simp only [WP.spec_ok]
      refine ⟨?_, hrnorm⟩
      rw [hrden, hgden]; simp
    | fail e => rw [hmkc] at hmk; exact hmk.elim
    | div => rw [hmkc] at hmk; exact hmk.elim
  | fail e => rw [hgc] at hg; exact hg.elim
  | div => rw [hgc] at hg; exact hg.elim

/-- **`int_divrem`** is truncated (toward-zero) division: quotient `Int.tdiv` (sign `a.neg XOR
    b.neg`), remainder `Int.tmod` (sign `a.neg`). `b ≠ 0`. -/
private theorem int_divrem_eq (a b : RefInt) (ha : IntNorm a) (hb : IntNorm b)
    (hb0 : den b.mag.limbs.val ≠ 0)
    (hbcap : b.mag.limbs.val.length + 1 ≤ Std.Usize.max)
    (hacap : a.mag.limbs.val.length * 64 ≤ Std.Usize.max) :
    int_divrem a b
      ⦃ r => iden r.1 = Int.tdiv (iden a) (iden b) ∧ iden r.2 = Int.tmod (iden a) (iden b)
          ∧ IntNorm r.1 ∧ IntNorm r.2 ⦄ := by
  unfold RefBackend.Insts.LatticeBackendBackendRefIntRefRat.int_divrem
  have hidenz : ¬ (iden b = 0) := by simp only [iden]; cases b.neg <;> simp [hb0]
  have hbz : decide (iden b = 0) = false := by simp only [decide_eq_false_iff_not]; exact hidenz
  rw [int_is_zero_eq b hb, hbz]
  show (do
      let (q, rr) ← RefNat.divrem a.mag b.mag
      let ri ← RefInt.make (a.neg != b.neg) q
      let ri1 ← RefInt.make a.neg rr
      ok (ri, ri1))
    ⦃ r => iden r.1 = Int.tdiv (iden a) (iden b) ∧ iden r.2 = Int.tmod (iden a) (iden b)
        ∧ IntNorm r.1 ∧ IntNorm r.2 ⦄
  have hdpos : 0 < den b.mag.limbs.val := Nat.pos_of_ne_zero hb0
  have hia : iden a = if a.neg then -(den a.mag.limbs.val : ℤ) else (den a.mag.limbs.val : ℤ) := by
    simp only [iden]
  have hib : iden b = if b.neg then -(den b.mag.limbs.val : ℤ) else (den b.mag.limbs.val : ℤ) := by
    simp only [iden]
  have hdr := divrem_eq a.mag b.mag ha.1 hb.1 hdpos hbcap hacap
  cases hdc : RefNat.divrem a.mag b.mag with
  | ok qr =>
    obtain ⟨q, rr⟩ := qr
    rw [hdc] at hdr; simp only [WP.spec_ok] at hdr
    obtain ⟨hqden, hrrden, hqnorm, hrrnorm⟩ := hdr
    -- the pure pair-let `let (q,rr) := (q,rr)` reduces definitionally; restate
    show (do
        let ri ← RefInt.make (a.neg != b.neg) q
        let ri1 ← RefInt.make a.neg rr
        ok (ri, ri1))
      ⦃ r => iden r.1 = Int.tdiv (iden a) (iden b) ∧ iden r.2 = Int.tmod (iden a) (iden b)
          ∧ IntNorm r.1 ∧ IntNorm r.2 ⦄
    have hmkq := make_spec (a.neg != b.neg) q hqnorm
    cases hmkqc : RefInt.make (a.neg != b.neg) q with
    | ok ri =>
      rw [hmkqc] at hmkq; simp only [WP.spec_ok] at hmkq
      obtain ⟨hqiden, hqinorm, _⟩ := hmkq
      simp only [bind_tc_ok]
      have hmkr := make_spec a.neg rr hrrnorm
      cases hmkrc : RefInt.make a.neg rr with
      | ok ri1 =>
        rw [hmkrc] at hmkr; simp only [WP.spec_ok] at hmkr
        obtain ⟨hriden, hrinorm, _⟩ := hmkr
        simp only [bind_tc_ok, WP.spec_ok]
        refine ⟨?_, ?_, hqinorm, hrinorm⟩
        · show iden ri = Int.tdiv (iden a) (iden b)
          rw [hqiden, hqden, hia, hib]
          cases a.neg <;> cases b.neg <;> simp [Int.neg_tdiv, Int.tdiv_neg]
        · show iden ri1 = Int.tmod (iden a) (iden b)
          have key : ∀ m n : ℕ, ((m : ℤ)).tmod (n : ℤ) = ((m % n : ℕ) : ℤ) := by
            intro m n; rw [Int.tmod_eq_emod_of_nonneg (by positivity)]; norm_cast
          rw [hriden, hrrden, hia, hib]
          cases a.neg <;> cases b.neg <;> simp [Int.neg_tmod, Int.tmod_neg, key]
      | fail e => rw [hmkrc] at hmkr; exact hmkr.elim
      | div => rw [hmkrc] at hmkr; exact hmkr.elim
    | fail e => rw [hmkqc] at hmkq; exact hmkq.elim
    | div => rw [hmkqc] at hmkq; exact hmkq.elim
  | fail e => rw [hdc] at hdr; exact hdr.elim
  | div => rw [hdc] at hdr; exact hdr.elim

set_option maxHeartbeats 400000 in
/-- **`int_lcm`** is `lcm(|a|, |b|) = |a|·|b| / gcd` as a nonnegative `RefInt` (`0` if either is `0`). -/
private theorem int_lcm_eq (a b : RefInt) (ha : IntNorm a) (hb : IntNorm b)
    (hacap : a.mag.limbs.val.length * 64 ≤ Std.Usize.max)
    (hbcap : b.mag.limbs.val.length * 64 ≤ Std.Usize.max)
    (hcap : (a.mag.limbs.val.length + b.mag.limbs.val.length) * 64 ≤ Std.Usize.max) :
    int_lcm a b
      ⦃ r => iden r = (Nat.lcm (den a.mag.limbs.val) (den b.mag.limbs.val) : ℤ) ∧ IntNorm r ⦄ := by
  unfold RefBackend.Insts.LatticeBackendBackendRefIntRefRat.int_lcm
  rw [int_is_zero_eq a ha]
  simp only [bind_tc_ok]
  by_cases haz : iden a = 0
  · rw [if_pos (by simp [haz])]
    have haz' : den a.mag.limbs.val = 0 := by
      have h := iden_natAbs a; rw [haz, Int.natAbs_zero] at h; exact h.symm
    have hz := int_zero_eq
    cases hzc : RefInt.zero with
    | ok r =>
      rw [hzc] at hz; simp only [WP.spec_ok] at hz
      obtain ⟨hziden, hznorm⟩ := hz
      simp only [WP.spec_ok]
      exact ⟨by rw [hziden, haz']; simp, hznorm⟩
    | fail e => rw [hzc] at hz; exact hz.elim
    | div => rw [hzc] at hz; exact hz.elim
  · rw [if_neg (by simp [haz])]
    rw [int_is_zero_eq b hb]
    simp only [bind_tc_ok]
    by_cases hbz : iden b = 0
    · rw [if_pos (by simp [hbz])]
      have hbz' : den b.mag.limbs.val = 0 := by
        have h := iden_natAbs b; rw [hbz, Int.natAbs_zero] at h; exact h.symm
      have hz := int_zero_eq
      cases hzc : RefInt.zero with
      | ok r =>
        rw [hzc] at hz; simp only [WP.spec_ok] at hz
        obtain ⟨hziden, hznorm⟩ := hz
        simp only [WP.spec_ok]
        exact ⟨by rw [hziden, hbz']; simp, hznorm⟩
      | fail e => rw [hzc] at hz; exact hz.elim
      | div => rw [hzc] at hz; exact hz.elim
    · rw [if_neg (by simp [hbz])]
      have haz' : den a.mag.limbs.val ≠ 0 := by
        rw [← iden_natAbs a]; exact Int.natAbs_ne_zero.mpr haz
      have hbz' : den b.mag.limbs.val ≠ 0 := by
        rw [← iden_natAbs b]; exact Int.natAbs_ne_zero.mpr hbz
      have hg := gcd_eq a.mag b.mag ha.1 hb.1 hacap hbcap
      cases hgc : RefNat.gcd a.mag b.mag with
      | ok g =>
        rw [hgc] at hg; simp only [WP.spec_ok] at hg
        obtain ⟨hgden, hgnorm⟩ := hg
        simp only [bind_tc_ok]
        have hgpos : 0 < den g.limbs.val := by
          rw [hgden]; exact Nat.gcd_pos_of_pos_left _ (Nat.pos_of_ne_zero haz')
        have hgle : den g.limbs.val ≤ den a.mag.limbs.val := by
          rw [hgden]; exact Nat.gcd_le_left _ (Nat.pos_of_ne_zero haz')
        have hglen : g.limbs.val.length ≤ a.mag.limbs.val.length := by
          by_contra hc; rw [not_le] at hc
          exact absurd (den_lt_of_len_lt a.mag.limbs.val g.limbs.val hgnorm hc) (by omega)
        have halen1 : 0 < a.mag.limbs.val.length :=
          List.length_pos_of_ne_nil (fun h => haz' ((den_eq_zero_iff _ ha.1).mpr h))
        have hmul := mul_eq a.mag b.mag (by omega)
        cases hmc : RefNat.mul a.mag b.mag with
        | ok rn =>
          rw [hmc] at hmul; simp only [WP.spec_ok] at hmul
          obtain ⟨hrnden, hrnnorm⟩ := hmul
          simp only [bind_tc_ok]
          have hrnlen := den_mul_len_le _ _ _ hrnnorm hrnden
          have hdr := divrem_eq rn g hrnnorm hgnorm hgpos (by omega) (by omega)
          cases hdc : RefNat.divrem rn g with
          | ok qr =>
            obtain ⟨q, rr⟩ := qr
            rw [hdc] at hdr; simp only [WP.spec_ok] at hdr
            obtain ⟨hqden, _, hqnorm, _⟩ := hdr
            show RefInt.make false q
              ⦃ r => iden r = (Nat.lcm (den a.mag.limbs.val) (den b.mag.limbs.val) : ℤ) ∧ IntNorm r ⦄
            have hmk := make_spec false q hqnorm
            cases hmkc : RefInt.make false q with
            | ok r =>
              rw [hmkc] at hmk; simp only [WP.spec_ok] at hmk
              obtain ⟨hriden, hrinorm, _⟩ := hmk
              simp only [WP.spec_ok]
              refine ⟨?_, hrinorm⟩
              rw [hriden, hqden, hrnden, hgden]
              simp [Nat.lcm]
            | fail e => rw [hmkc] at hmk; exact hmk.elim
            | div => rw [hmkc] at hmk; exact hmk.elim
          | fail e => rw [hdc] at hdr; exact hdr.elim
          | div => rw [hdc] at hdr; exact hdr.elim
        | fail e => rw [hmc] at hmul; exact hmul.elim
        | div => rw [hmc] at hmul; exact hmul.elim
      | fail e => rw [hgc] at hg; exact hg.elim
      | div => rw [hgc] at hg; exact hg.elim

-- Axiom audit: the op refinements are axiom-clean (no cited axiom, no `sorryAx` — the Aeneas
-- Std `get_unchecked`/`Slice` sorries are off these paths).
#print axioms is_zero_eq
#print axioms cmp_eq
#print axioms add_eq
#print axioms sub_eq
#print axioms mul_eq
#print axioms shl1_eq
#print axioms testbit_eq
#print axioms bit_len_spec
#print axioms divrem_eq
#print axioms gcd_eq
#print axioms int_is_zero_eq
#print axioms rat_neg_eq
#print axioms rat_numer_eq
#print axioms rat_denom_eq
#print axioms rat_is_zero_eq
#print axioms rat_sign_eq
#print axioms rat_cmp_eq
#print axioms rat_from_ints_eq
#print axioms rat_mul_eq
#print axioms rat_div_eq
#print axioms rat_add_eq
#print axioms rat_sub_eq
#print axioms int_zero_backend_eq
#print axioms int_add_backend_eq
#print axioms int_sub_backend_eq
#print axioms int_mul_backend_eq
#print axioms int_neg_backend_eq
#print axioms int_cmp_backend_eq
#print axioms int_sign_backend_eq
#print axioms int_is_zero_backend_eq
#print axioms int_gcd_eq
#print axioms int_divrem_eq
#print axioms int_lcm_eq

end CertifyCheck.RefBackend
