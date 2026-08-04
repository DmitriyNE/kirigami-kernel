/-
  CLIP-σ signed disjunction — the ★ soundness row of the M2 transversality ladder (spec §8.5),
  now **derived** from the Aeneas-lifted `certify1d::clip_sigma` over Mathlib ℚ (algebra-rehaul
  R.3c). This replaces the former hand-written ℤ mirror: the ★-critical *decision*
  `clip_sigma_branch` is proved to **equal** its mathematical spec (`clip_sigma_branch_eq`) by
  reducing the *extracted* Rust body — so a change to the running Rust surfaces as a broken
  refinement proof rather than silent spec drift. The range `corner_range` refinement (a slice-loop
  proof) is tracked as R.3c-cont (see the note near the end); its spec is proved sound here. The
  soundness statements are over ℚ, matching the lifted code.

  `certify1d::clip_sigma` certifies that the *signed* affine `∂_σG` is single-signed and separated
  across a `(μ, w)` box by ranging it over the four corners. The row exists because the tempting
  *squared* corner test is unsound: an affine form minimizes in the box interior, so `G = σμ`
  (`∂_σG = μ`, zero on `μ = 0`) passes `|∂_σG|² ≥ m` with a margin while the crossing is singular.
  A certified verdict forces *every* corner strictly single-signed and separated, so the `σμ` class
  (a mixed-sign corner set) is rejected — proved below about the lifted code. Axiom-clean
  (`#print axioms` at the end).
-/
import Mathlib
import CertifyCore.Funs
import CertifyCheck.Refine

namespace CertifyCheck.ClipSigma
open Aeneas Aeneas.Std Result certify_core certify_core.certify1d

/-- The certified single sign of the affine trim form — the extracted `certify1d::ClipBranch`. -/
abbrev Branch := certify1d.ClipBranch

/-! ### Mathematical spec (the reduced form of the extracted decision, over ℚ) -/

/-- The CLIP-σ signed-disjunction decision: `Positive` iff `m ≤ lo`, else `Negative` iff
    `hi ≤ neg_m`, both gated on `m_positive` (`clip_sigma` passes `neg_m = -m` and
    `m_positive = 0 < m` precomputed, to stay generic over the ordered type). -/
def clipSigmaBranchSpec (lo hi m neg_m : ℚ) (mPositive : Bool) : Option Branch :=
  if mPositive then
    if m ≤ lo then some .Positive
    else if hi ≤ neg_m then some .Negative
    else none
  else none

/-- The corner range: the `(min, max)` of a nonempty corner list, `none` when empty. -/
def cornerRangeSpec : List ℚ → Option (ℚ × ℚ)
  | [] => none
  | c :: cs => some (cs.foldl min c, cs.foldl max c)

/-! ### The decision spec is sound (pure branch, no range) -/

theorem branch_positive_sound {lo hi m neg_m : ℚ} {mp : Bool}
    (h : clipSigmaBranchSpec lo hi m neg_m mp = some ClipBranch.Positive) : mp = true ∧ m ≤ lo := by
  unfold clipSigmaBranchSpec at h
  by_cases hm : mp
  · by_cases hlo : m ≤ lo
    · exact ⟨hm, hlo⟩
    · simp [hm, hlo] at h
  · simp [hm] at h

theorem branch_negative_sound {lo hi m neg_m : ℚ} {mp : Bool}
    (h : clipSigmaBranchSpec lo hi m neg_m mp = some ClipBranch.Negative) : mp = true ∧ hi ≤ neg_m := by
  unfold clipSigmaBranchSpec at h
  by_cases hm : mp
  · by_cases hlo : m ≤ lo
    · simp [hm, hlo] at h
    · by_cases hhi : hi ≤ neg_m
      · exact ⟨hm, hhi⟩
      · simp [hm, hlo, hhi] at h
  · simp [hm] at h

/-! ### The corner range brackets every corner -/

private theorem foldl_min_le_acc (c : ℚ) (cs : List ℚ) : cs.foldl min c ≤ c := by
  induction cs generalizing c with
  | nil => simp
  | cons a t ih => exact le_trans (ih (min c a)) (min_le_left c a)

private theorem foldl_min_le_mem (c : ℚ) (cs : List ℚ) : ∀ x ∈ cs, cs.foldl min c ≤ x := by
  induction cs generalizing c with
  | nil => simp
  | cons a t ih =>
    intro x hx
    rcases List.mem_cons.mp hx with h | h
    · rw [h]; exact le_trans (foldl_min_le_acc (min c a) t) (min_le_right c a)
    · exact ih (min c a) x h

private theorem le_foldl_max_acc (c : ℚ) (cs : List ℚ) : c ≤ cs.foldl max c := by
  induction cs generalizing c with
  | nil => simp
  | cons a t ih => exact le_trans (le_max_left c a) (ih (max c a))

private theorem mem_le_foldl_max (c : ℚ) (cs : List ℚ) : ∀ x ∈ cs, x ≤ cs.foldl max c := by
  induction cs generalizing c with
  | nil => simp
  | cons a t ih =>
    intro x hx
    rcases List.mem_cons.mp hx with h | h
    · rw [h]; exact le_trans (le_max_right c a) (le_foldl_max_acc (max c a) t)
    · exact ih (max c a) x h

theorem cornerRange_lower {cs : List ℚ} {lo hi : ℚ}
    (hr : cornerRangeSpec cs = some (lo, hi)) : ∀ c ∈ cs, lo ≤ c := by
  cases cs with
  | nil => simp [cornerRangeSpec] at hr
  | cons a t =>
    simp only [cornerRangeSpec, Option.some.injEq, Prod.mk.injEq] at hr
    obtain ⟨hlo, _⟩ := hr
    intro x hx
    rw [← hlo]
    rcases List.mem_cons.mp hx with h | h
    · rw [h]; exact foldl_min_le_acc a t
    · exact foldl_min_le_mem a t x h

theorem cornerRange_upper {cs : List ℚ} {lo hi : ℚ}
    (hr : cornerRangeSpec cs = some (lo, hi)) : ∀ c ∈ cs, c ≤ hi := by
  cases cs with
  | nil => simp [cornerRangeSpec] at hr
  | cons a t =>
    simp only [cornerRangeSpec, Option.some.injEq, Prod.mk.injEq] at hr
    obtain ⟨_, hhi⟩ := hr
    intro x hx
    rw [← hhi]
    rcases List.mem_cons.mp hx with h | h
    · rw [h]; exact le_foldl_max_acc a t
    · exact mem_le_foldl_max a t x h

/-! ### The ★ soundness statement about the spec (range ∘ branch) -/

/-- Positive branch: certifies **every** corner is separated above zero by `m` (`∂_σG ≥ m`),
    given the caller's `neg_m = -m` and `mPositive = (0 < m)`. -/
theorem clipSigma_sound_positive {cs : List ℚ} {lo hi m : ℚ}
    (hr : cornerRangeSpec cs = some (lo, hi))
    (hb : clipSigmaBranchSpec lo hi m (-m) (decide (0 < m)) = some ClipBranch.Positive) :
    0 < m ∧ ∀ c ∈ cs, m ≤ c := by
  obtain ⟨hm, hlo⟩ := branch_positive_sound hb
  exact ⟨by simpa using hm, fun c hc => le_trans hlo (cornerRange_lower hr c hc)⟩

/-- Negative branch: certifies **every** corner is separated below zero by `m` (`∂_σG ≤ -m`). -/
theorem clipSigma_sound_negative {cs : List ℚ} {lo hi m : ℚ}
    (hr : cornerRangeSpec cs = some (lo, hi))
    (hb : clipSigmaBranchSpec lo hi m (-m) (decide (0 < m)) = some ClipBranch.Negative) :
    0 < m ∧ ∀ c ∈ cs, c ≤ -m := by
  obtain ⟨hm, hhi⟩ := branch_negative_sound hb
  exact ⟨by simpa using hm, fun c hc => le_trans (cornerRange_upper hr c hc) hhi⟩

/-- The `σμ` falsely-certifying class is rejected: a corner set with a strictly positive **and** a
    strictly negative corner is never certified (any margin) — the affine range straddles zero. -/
theorem clipSigma_rejects_straddle {cs : List ℚ} {lo hi m : ℚ}
    (hr : cornerRangeSpec cs = some (lo, hi))
    (hpos : ∃ c ∈ cs, 0 < c) (hneg : ∃ c ∈ cs, c < 0) :
    clipSigmaBranchSpec lo hi m (-m) (decide (0 < m)) = none := by
  obtain ⟨cp, hcp, hcppos⟩ := hpos
  obtain ⟨cn, hcn, hcnneg⟩ := hneg
  have hlo : lo ≤ cn := cornerRange_lower hr cn hcn
  have hhi : cp ≤ hi := cornerRange_upper hr cp hcp
  unfold clipSigmaBranchSpec
  by_cases hm : 0 < m
  · have h1 : ¬ m ≤ lo := by linarith
    have h2 : ¬ hi ≤ -m := by linarith
    simp [hm, h1, h2]
  · simp [hm]

/-! ### The refinements: the extracted Rust body equals the spec (the drift-kill) -/

/-- **Branch refinement.** The Aeneas-lifted `clip_sigma_branch`, at `T = ℚ` with the lifted `Ord`
    instance (`compare`), computes `clipSigmaBranchSpec`. -/
theorem clip_sigma_branch_eq {B I R : Type} (inst : lattice.backend.Backend B I R)
    (lo hi m neg_m : ℚ) (mp : Bool) :
    certify1d.clip_sigma_branch (lattice.rat.Rat.Insts.CoreCmpOrd inst) lo hi m neg_m mp
      = ok (clipSigmaBranchSpec lo hi m neg_m mp) := by
  -- Reduce the extracted body: `.cmp` = `compare`, `ne` = `!eq`, then `compare _ _ = .lt ↔ <`.
  simp [certify1d.clip_sigma_branch, lattice.rat.Rat.Insts.CoreCmpOrd.cmp,
    core.cmp.PartialEq.ne.trait_default, core.cmp.PartialEq.ne.default,
    core.cmp.Ordering.Insts.CoreCmpPartialEqOrdering.eq, clipSigmaBranchSpec, compare_lt_iff_lt,
    compare_gt_iff_gt]
  split_ifs <;> first | rfl | (exfalso; linarith)

/-
  **Range refinement (tracked, R.3c-cont).** The companion `corner_range` refinement —
    `certify1d.corner_range (CoreCmpOrd inst) (CoreCloneClone inst) corners
       = ok (cornerRangeSpec corners.val)`
  — is a slice-loop proof of the same shape as `Refine.lean`'s `sign_variations_spec`:
  `loop.spec_decr_nat` over `corner_range_loop` with `measure := len − it.i` and invariant
  `(corners.val.drop it.i).foldl min lo = (corners.val.drop it0.i).foldl min lo0` (and `max`/`hi`),
  driven by the reusable `@[step] sliceIter_next_spec`; the body step reduces `.cmp` to `compare`
  and `lo1 = min lo c` / `hi1 = max hi c` via `compare_lt_iff_lt` + `List.foldl_cons`, and the
  prelude reduces `Slice.iter`/`next`/`branch` before invoking the loop spec. Landing it lifts
  `cornerRangeSpec` from a (sound, but hand-written) spec to a derived one — completing the CLIP-σ
  drift-kill. The ★-critical decision (`clip_sigma_branch_eq`) is already derived below.
-/

-- Axiom audit: the derived decision + the spec-soundness are axiom-clean (no cited axiom, no
-- `sorryAx` — in particular the Aeneas Std `get_unchecked` sorries are off this path).
#print axioms clip_sigma_branch_eq
#print axioms clipSigma_sound_positive
#print axioms clipSigma_sound_negative
#print axioms clipSigma_rejects_straddle

end CertifyCheck.ClipSigma
