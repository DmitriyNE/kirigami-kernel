/-
  CLIP-σ signed disjunction — the ★ soundness row of the M2 transversality ladder (spec §8.5),
  now **derived** from the Aeneas-lifted `certify1d::clip_sigma` over Mathlib ℚ (algebra-rehaul
  R.3c). This replaces the former hand-written ℤ mirror: BOTH the ★-critical *decision*
  `clip_sigma_branch` (`clip_sigma_branch_eq`) and the *range* `corner_range` (`corner_range_eq`,
  a `loop.spec_decr_nat` slice-loop refinement) are proved to **equal** their mathematical spec by
  reducing the *extracted* Rust body — so a change to the running Rust surfaces as a broken
  refinement proof rather than silent spec drift. The soundness statements are over ℚ, matching the
  lifted code.

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
theorem clip_sigma_branch_eq {B I R : Type} (inst : certify_core.lattice.backend.Backend B I R)
    (lo hi m neg_m : ℚ) (mp : Bool) :
    certify1d.clip_sigma_branch (lattice.rat.Rat.Insts.CoreCmpOrd inst) lo hi m neg_m mp
      = ok (clipSigmaBranchSpec lo hi m neg_m mp) := by
  -- Reduce the extracted body: `.cmp` = `compare`, `ne` = `!eq`, then `compare _ _ = .lt ↔ <`.
  simp [certify1d.clip_sigma_branch, lattice.rat.Rat.Insts.CoreCmpOrd.cmp,
    core.cmp.PartialEq.ne.trait_default, core.cmp.PartialEq.ne.default,
    core.cmp.Ordering.Insts.CoreCmpPartialEqOrdering.eq, clipSigmaBranchSpec, compare_lt_iff_lt,
    compare_gt_iff_gt]
  split_ifs <;> first | rfl | (exfalso; linarith)

/-! ### Range refinement: the extracted `corner_range` loop computes `cornerRangeSpec` -/

/-- The loop folds `min`/`max` over the remaining slice onto the running `(lo, hi)`. Proved the
    Aeneas way — `loop.spec_decr_nat` + the reusable `@[step] sliceIter_next_spec` (`Refine.lean`)
    — with the "answer-preserving" invariant `(drop i).foldl min lo = (drop i₀).foldl min lo₀`. -/
private theorem corner_range_loop_spec {B I R : Type} (inst : certify_core.lattice.backend.Backend B I R)
    (corners : Slice ℚ) (it0 : core.slice.iter.Iter ℚ) (lo0 hi0 : ℚ)
    (hsl0 : it0.slice = corners) (hle0 : it0.i ≤ corners.val.length) :
    certify1d.corner_range_loop (lattice.rat.Rat.Insts.CoreCmpOrd inst)
        (lattice.rat.Rat.Insts.CoreCloneClone inst) it0 lo0 hi0
      ⦃ r => r.1 = (corners.val.drop it0.i).foldl min lo0 ∧
             r.2 = (corners.val.drop it0.i).foldl max hi0 ⦄ := by
  unfold certify1d.corner_range_loop
  apply loop.spec_decr_nat
    (measure := fun st => corners.val.length - st.1.i)
    (inv := fun st => st.1.slice = corners ∧ st.1.i ≤ corners.val.length ∧
      (corners.val.drop st.1.i).foldl min st.2.1 = (corners.val.drop it0.i).foldl min lo0 ∧
      (corners.val.drop st.1.i).foldl max st.2.2 = (corners.val.drop it0.i).foldl max hi0)
  · rintro ⟨it, lo, hi⟩ ⟨hslice, hle, hmin, hmax⟩
    simp only [] at hslice hle hmin hmax
    show certify1d.corner_range_loop.body _ _ it lo hi ⦃ _ ⦄
    unfold certify1d.corner_range_loop.body
    step as ⟨opt, it', hnext, hsleq⟩
    have hlenC : it.slice.val.length = corners.val.length := by rw [hslice]
    by_cases hlt : it.i < it.slice.val.length
    · rw [if_pos hlt] at hnext
      obtain ⟨hopt, hi'⟩ := hnext
      rw [hopt]; simp only []
      have hltC : it.i < corners.val.length := hlenC ▸ hlt
      set c := it.slice.val[it.i]! with hcdef
      have hcval : c = corners.val[it.i]'hltC := by
        rw [hcdef, hslice, getElem!_pos corners.val it.i (hslice ▸ hlt)]
      have hdrop : corners.val.drop it.i = c :: corners.val.drop it'.i := by
        rw [hi', hcval]; exact (List.getElem_cons_drop hltC).symm
      have hisl : it'.slice = corners := by rw [hsleq, hslice]
      have hidec : it'.i ≤ corners.val.length := by rw [hi']; omega
      have hmeas : corners.val.length - it'.i < corners.val.length - it.i := by rw [hi']; omega
      have hminpres : (corners.val.drop it'.i).foldl min (min lo c)
          = (corners.val.drop it0.i).foldl min lo0 := by rw [← hmin, hdrop, List.foldl_cons]
      have hmaxpres : (corners.val.drop it'.i).foldl max (max hi c)
          = (corners.val.drop it0.i).foldl max hi0 := by rw [← hmax, hdrop, List.foldl_cons]
      -- the body updates `lo := min lo c`, `hi := max hi c` and continues at `it'`.
      have hlo1 : (if c < lo then c else lo) = min lo c := by
        split_ifs with h
        · exact (min_eq_right h.le).symm
        · exact (min_eq_left (not_lt.mp h)).symm
      have hhi1 : (if hi < c then c else hi) = max hi c := by
        split_ifs with h
        · exact (max_eq_right h.le).symm
        · exact (max_eq_left (not_lt.mp h)).symm
      simp only [lattice.rat.Rat.Insts.CoreCmpOrd.cmp, lattice.rat.Rat.Insts.CoreCloneClone.clone,
        core.cmp.Ordering.Insts.CoreCmpPartialEqOrdering.eq, compare_lt_iff_lt, compare_gt_iff_gt,
        decide_eq_true_eq, ← apply_ite ok, hlo1, WP.spec_ok, bind_tc_ok]
      split_ifs with hchi
      · exact ⟨hisl, hidec, hminpres, by rw [max_eq_right hchi.le] at hmaxpres; exact hmaxpres, hmeas⟩
      · exact ⟨hisl, hidec, hminpres,
          by rw [max_eq_left (not_lt.mp hchi)] at hmaxpres; exact hmaxpres, hmeas⟩
    · rw [if_neg hlt] at hnext
      obtain ⟨hopt, _⟩ := hnext
      rw [hopt]; simp only []
      have hge : ¬ it.i < corners.val.length := hlenC ▸ hlt
      have hieq : it.i = corners.val.length := le_antisymm hle (Nat.le_of_not_lt hge)
      rw [hieq, List.drop_length, List.foldl_nil] at hmin hmax
      exact ⟨hmin, hmax⟩
  · exact ⟨hsl0, hle0, rfl, rfl⟩

/-- **Range refinement.** The Aeneas-lifted `corner_range`, at `T = ℚ`, computes `cornerRangeSpec`
    on the underlying list of corners — so `cornerRangeSpec` is now derived, not a hand mirror. -/
theorem corner_range_eq {B I R : Type} (inst : certify_core.lattice.backend.Backend B I R) (corners : Slice ℚ) :
    certify1d.corner_range (lattice.rat.Rat.Insts.CoreCmpOrd inst)
        (lattice.rat.Rat.Insts.CoreCloneClone inst) corners
      ⦃ r => r = cornerRangeSpec corners.val ⦄ := by
  unfold certify1d.corner_range
  simp only [core.slice.Slice.iter, core.option.Option.Insts.CoreOpsTry_traitTry.branch,
    lattice.rat.Rat.Insts.CoreCloneClone.clone, bind_tc_ok]
  step as ⟨opt, it', hnext, hsleq⟩
  by_cases hlen : 0 < corners.val.length
  · rw [if_pos hlen] at hnext
    obtain ⟨hopt, hit'⟩ := hnext
    rw [hopt]
    simp only []
    have h0 : (0 : ℕ) < corners.val.length := by omega
    have hc0 : corners.val = corners.val[0]! :: corners.val.drop it'.i := by
      rw [hit', getElem!_pos corners.val 0 h0]
      conv_lhs => rw [← List.drop_zero (l := corners.val)]
      exact (List.getElem_cons_drop h0).symm
    have hloop := corner_range_loop_spec inst corners it' (corners.val[0]!) (corners.val[0]!)
      hsleq (by omega)
    -- the loop succeeds with the min/max fold over the tail; assemble `some (·)` = cornerRangeSpec.
    cases hcase : corner_range_loop (lattice.rat.Rat.Insts.CoreCmpOrd inst)
        (lattice.rat.Rat.Insts.CoreCloneClone inst) it' (corners.val[0]!) (corners.val[0]!) with
    | ok r =>
      obtain ⟨lo1, hi⟩ := r
      rw [hcase] at hloop
      simp only [WP.spec_ok] at hloop
      obtain ⟨hlo1eq, hhi1eq⟩ := hloop
      rw [hc0]
      simp [bind_tc_ok, WP.spec_ok, cornerRangeSpec, hlo1eq, hhi1eq]
    | fail e => rw [hcase] at hloop; exact hloop.elim
    | div => rw [hcase] at hloop; exact hloop.elim
  · rw [if_neg hlen] at hnext
    obtain ⟨hopt, _⟩ := hnext
    rw [hopt]
    have hnil : corners.val = [] := List.length_eq_zero_iff.mp (by omega)
    simp only [core.option.Option.Insts.CoreOpsTry_traitFromResidualOptionInfallible.from_residual,
      WP.spec_ok, hnil, cornerRangeSpec]

-- Axiom audit: the derived decision/range + the spec-soundness are axiom-clean (no cited axiom, no
-- `sorryAx` — in particular the Aeneas Std `get_unchecked` sorries are off this path).
#print axioms clip_sigma_branch_eq
#print axioms corner_range_eq
#print axioms clipSigma_sound_positive
#print axioms clipSigma_sound_negative
#print axioms clipSigma_rejects_straddle

end CertifyCheck.ClipSigma
