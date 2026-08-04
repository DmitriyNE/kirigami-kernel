/-
  CLIP-σ signed disjunction — the ★ soundness row of the M2 transversality ladder
  (spec §8.5), as an unbounded-ℤ deductive witness complementing the bounded Kani proof
  `certify_core::proof::clip_sigma_signed_disjunction_sound`.

  `certify1d::clip_sigma` certifies that the *signed* affine `∂_σG` is single-signed and
  separated across a `(μ, w)` box by ranging it over the four corners. The row exists
  because the tempting *squared* corner test is unsound: an affine form minimizes in the
  box interior, so `G = σμ` (`∂_σG = μ`, zero on `μ = 0`) passes `|∂_σG|² ≥ m` with a
  margin while the crossing is singular. This file proves the signed test does not — a
  certified verdict forces *every* corner strictly single-signed and separated, so any
  mixed-sign corner set (the `σμ` class) is rejected.

  This is a **hand-written** spec over Mathlib ℤ, mirroring the Rust decision core
  `certify1d::clip_sigma_branch` / `corner_range` field-for-field. Unlike the Aeneas-lifted
  `CapOut` specs it is not extracted: the public `clip_sigma` bottoms out in `lattice::Rat`
  arithmetic (dashu), which the pinned Aeneas cannot lift — the algebra-lift wall folded
  into the post-B `Int = ℤ / Rat = ℚ` rehaul (`docs/algebra-trust.md`). Fidelity to the
  running code is anchored by the Kani harness, which executes the *actual* Rust
  `clip_sigma_branch`; this file adds unbounded-domain deductive confidence. No axiom is
  cited — each theorem is proved about the mirrored function (`#print axioms` below).
-/
import Mathlib

namespace CertifyCheck.ClipSigma

/-- A certified single sign of the affine trim form (Rust `certify1d::ClipBranch`). -/
inductive Branch
  | positive
  | negative
  deriving DecidableEq, Repr

/-- The CLIP-σ signed-disjunction decision, mirroring `certify1d::clip_sigma_branch`:
    `positive` iff `lo ≥ m`, `negative` iff `hi ≤ -m`, both gated on `m > 0`; else `none`.
    (Rust passes `neg_m = -m` and `m_positive = 0 < m` precomputed, to stay generic over
    the ordered type; over ℤ they are inlined.) -/
def clipSigmaBranch (lo hi m : ℤ) : Option Branch :=
  if 0 < m then
    if m ≤ lo then some .positive
    else if hi ≤ -m then some .negative
    else none
  else none

/-- The corner range, mirroring `certify1d::corner_range`: the `(min, max)` of a nonempty
    corner list, `none` when empty. -/
def cornerRange : List ℤ → Option (ℤ × ℤ)
  | [] => none
  | c :: cs => some (cs.foldl min c, cs.foldl max c)

/-! ### The decision is sound (pure branch, no range) -/

/-- A `positive` verdict means the threshold is a real separation and the low corner
    clears it. -/
theorem clipSigmaBranch_positive_sound {lo hi m : ℤ}
    (h : clipSigmaBranch lo hi m = some Branch.positive) : 0 < m ∧ m ≤ lo := by
  unfold clipSigmaBranch at h
  by_cases hm : 0 < m
  · by_cases hlo : m ≤ lo
    · exact ⟨hm, hlo⟩
    · simp [hm, hlo] at h
  · simp [hm] at h

/-- A `negative` verdict means the threshold is a real separation and the high corner
    clears `-m`. -/
theorem clipSigmaBranch_negative_sound {lo hi m : ℤ}
    (h : clipSigmaBranch lo hi m = some Branch.negative) : 0 < m ∧ hi ≤ -m := by
  unfold clipSigmaBranch at h
  by_cases hm : 0 < m
  · by_cases hlo : m ≤ lo
    · simp [hm, hlo] at h
    · by_cases hhi : hi ≤ -m
      · exact ⟨hm, hhi⟩
      · simp [hm, hlo, hhi] at h
  · simp [hm] at h

/-! ### The corner range brackets every corner -/

private theorem foldl_min_le_acc (c : ℤ) (cs : List ℤ) : cs.foldl min c ≤ c := by
  induction cs generalizing c with
  | nil => simp
  | cons a t ih => exact le_trans (ih (min c a)) (min_le_left c a)

private theorem foldl_min_le_mem (c : ℤ) (cs : List ℤ) :
    ∀ x ∈ cs, cs.foldl min c ≤ x := by
  induction cs generalizing c with
  | nil => simp
  | cons a t ih =>
    intro x hx
    rcases List.mem_cons.mp hx with h | h
    · rw [h]; exact le_trans (foldl_min_le_acc (min c a) t) (min_le_right c a)
    · exact ih (min c a) x h

private theorem le_foldl_max_acc (c : ℤ) (cs : List ℤ) : c ≤ cs.foldl max c := by
  induction cs generalizing c with
  | nil => simp
  | cons a t ih => exact le_trans (le_max_left c a) (ih (max c a))

private theorem mem_le_foldl_max (c : ℤ) (cs : List ℤ) :
    ∀ x ∈ cs, x ≤ cs.foldl max c := by
  induction cs generalizing c with
  | nil => simp
  | cons a t ih =>
    intro x hx
    rcases List.mem_cons.mp hx with h | h
    · rw [h]; exact le_trans (le_max_right c a) (le_foldl_max_acc (max c a) t)
    · exact ih (max c a) x h

/-- `lo` from `cornerRange` is a lower bound on every corner. -/
theorem cornerRange_lower {cs : List ℤ} {lo hi : ℤ}
    (hr : cornerRange cs = some (lo, hi)) : ∀ c ∈ cs, lo ≤ c := by
  cases cs with
  | nil => simp [cornerRange] at hr
  | cons a t =>
    simp only [cornerRange, Option.some.injEq, Prod.mk.injEq] at hr
    obtain ⟨hlo, _⟩ := hr
    intro x hx
    rw [← hlo]
    rcases List.mem_cons.mp hx with h | h
    · rw [h]; exact foldl_min_le_acc a t
    · exact foldl_min_le_mem a t x h

/-- `hi` from `cornerRange` is an upper bound on every corner. -/
theorem cornerRange_upper {cs : List ℤ} {lo hi : ℤ}
    (hr : cornerRange cs = some (lo, hi)) : ∀ c ∈ cs, c ≤ hi := by
  cases cs with
  | nil => simp [cornerRange] at hr
  | cons a t =>
    simp only [cornerRange, Option.some.injEq, Prod.mk.injEq] at hr
    obtain ⟨_, hhi⟩ := hr
    intro x hx
    rw [← hhi]
    rcases List.mem_cons.mp hx with h | h
    · rw [h]; exact le_foldl_max_acc a t
    · exact mem_le_foldl_max a t x h

/-! ### The ★ soundness statement (range ∘ branch) -/

/-- CLIP-σ soundness, positive branch: a `positive` verdict certifies that **every** corner
    is separated above zero by `m` (so `∂_σG ≥ m > 0` across the affine box). -/
theorem clipSigma_sound_positive {cs : List ℤ} {lo hi m : ℤ}
    (hr : cornerRange cs = some (lo, hi))
    (hb : clipSigmaBranch lo hi m = some Branch.positive) :
    0 < m ∧ ∀ c ∈ cs, m ≤ c := by
  obtain ⟨hm, hlo⟩ := clipSigmaBranch_positive_sound hb
  exact ⟨hm, fun c hc => le_trans hlo (cornerRange_lower hr c hc)⟩

/-- CLIP-σ soundness, negative branch: a `negative` verdict certifies that **every** corner
    is separated below zero by `m` (so `∂_σG ≤ -m < 0` across the affine box). -/
theorem clipSigma_sound_negative {cs : List ℤ} {lo hi m : ℤ}
    (hr : cornerRange cs = some (lo, hi))
    (hb : clipSigmaBranch lo hi m = some Branch.negative) :
    0 < m ∧ ∀ c ∈ cs, c ≤ -m := by
  obtain ⟨hm, hhi⟩ := clipSigmaBranch_negative_sound hb
  exact ⟨hm, fun c hc => le_trans (cornerRange_upper hr c hc) hhi⟩

/-- The `σμ` falsely-certifying class is rejected: a corner set with a strictly positive
    **and** a strictly negative corner is never certified (any margin) — the affine range
    straddles zero, exactly where a squared corner test would falsely `Verify`. -/
theorem clipSigma_rejects_straddle {cs : List ℤ} {lo hi m : ℤ}
    (hr : cornerRange cs = some (lo, hi))
    (hpos : ∃ c ∈ cs, 0 < c) (hneg : ∃ c ∈ cs, c < 0) :
    clipSigmaBranch lo hi m = none := by
  obtain ⟨cp, hcp, hcppos⟩ := hpos
  obtain ⟨cn, hcn, hcnneg⟩ := hneg
  have hlo : lo ≤ cn := cornerRange_lower hr cn hcn
  have hhi : cp ≤ hi := cornerRange_upper hr cp hcp
  unfold clipSigmaBranch
  by_cases hm : 0 < m
  · have h1 : ¬ m ≤ lo := by omega
    have h2 : ¬ hi ≤ -m := by omega
    simp [hm, h1, h2]
  · simp [hm]

-- The ★ soundness theorems are axiom-clean (no Sturm-style cited axiom): the decision is
-- proved directly about the mirrored function.
#print axioms clipSigma_sound_positive
#print axioms clipSigma_sound_negative
#print axioms clipSigma_rejects_straddle

end CertifyCheck.ClipSigma
