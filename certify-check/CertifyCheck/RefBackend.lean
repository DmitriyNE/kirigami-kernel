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

-- Axiom audit: the two op refinements are axiom-clean (no cited axiom, no `sorryAx` — the Aeneas
-- Std `get_unchecked`/`Slice` sorries are off these paths).
#print axioms is_zero_eq
#print axioms cmp_eq

end CertifyCheck.RefBackend
