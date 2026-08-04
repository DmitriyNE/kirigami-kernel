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
  invariant every constructor/op re-establishes via `normalize`. It is carried here as an explicit
  `Normalized` hypothesis; a later phase proves `normalize` produces it.
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
    ⦃ r => den r.limbs.val = den a.limbs.val + den b.limbs.val ⦄ := by
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
          ⦃ r => den r.limbs.val = den a.limbs.val + den b.limbs.val ⦄ := by
      intro out2 hout2den
      have hnorm := normalize_den out2
      cases hnc : lattice.refbackend.normalize out2 with
      | ok o =>
        rw [hnc] at hnorm; simp only [WP.spec_ok] at hnorm
        simp only [bind_tc_ok, WP.spec_ok]
        show den o.val = _; rw [hnorm, hout2den]
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
    RefNat.add a b ⦃ r => den r.limbs.val = den a.limbs.val + den b.limbs.val ⦄ := by
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
    RefNat.sub self o ⦃ r => den r.limbs.val = den self.limbs.val - den o.limbs.val ⦄ := by
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
    cases hnc : lattice.refbackend.normalize out1 with
    | ok o2 =>
      rw [hnc] at hnorm; simp only [WP.spec_ok] at hnorm
      simp only [bind_tc_ok, WP.spec_ok]
      show den o2.val = _; rw [hnorm]; omega
    | fail e => rw [hnc] at hnorm; exact hnorm.elim
    | div => rw [hnc] at hnorm; exact hnorm.elim
  | fail e => rw [hcase] at hloop; exact hloop.elim
  | div => rw [hcase] at hloop; exact hloop.elim

-- Axiom audit: the op refinements are axiom-clean (no cited axiom, no `sorryAx` — the Aeneas
-- Std `get_unchecked`/`Slice` sorries are off these paths).
#print axioms is_zero_eq
#print axioms cmp_eq
#print axioms add_eq
#print axioms sub_eq

end CertifyCheck.RefBackend
