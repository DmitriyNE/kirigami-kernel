/-
  Slice 3e — the CAP-OUT-LINK run-counter refinement (finishing the 3e.5 frontier).

  The Aeneas-lifted `arrange.cyclic_true_runs` (a `Result`-monadic loop over the sector
  slice with cyclic modular indexing) computes the mathematical **cyclic-run count** of
  the sector mask; hence `link_ok` accepts iff that count is `≤ 1` — the CAP-OUT-LINK
  manifold rule, now a *deductive* Lean theorem over the extracted model (matching the
  bounded Kani proof `link_ok_iff_no_pinch`, spec §8.5).

  Proven the intended Aeneas way — `loop.spec_decr_nat` for the loop + `step` for the
  body (measure `n − i`; the forward-count invariant `runs = runsUpTo i`; the `Usize`
  overflow of `i + n` discharged by `2·len ≤ Usize.max`, the slice-length analogue of
  `sign_variations_spec`'s `hlen`) — mirroring `GcdReduce.gcd_u128_spec`.
-/
import CertifyCore.Funs

open Aeneas Aeneas.Std Result

namespace CertifyCheck
open certify_core

/-! ### The mathematical spec -/

/-- A false→true cyclic transition at sector `i` (= the start of a run of `true`):
    sector `i` is selected and its cyclic predecessor `(i + n − 1) % n` is not. Matches
    the Rust body's `sectors[i] && !sectors[(i + n − 1) % n]` verbatim. -/
def transAt (l : List Bool) (i : Nat) : Bool :=
  l[i]! && !l[(i + l.length - 1) % l.length]!

/-- The number of cyclic runs (= false→true transitions) among the first `k` sectors. -/
def runsUpTo (l : List Bool) (k : Nat) : Nat := (List.range k).countP (transAt l)

/-- The transition count over all sectors — what `cyclic_true_runs`'s loop returns. -/
def numRuns (l : List Bool) : Nat := runsUpTo l l.length

/-- The full `cyclic_true_runs` value: the transition count, with the all-`true` ring
    (no transition but one full run) fixed up to `1`. -/
def cyclicRuns (l : List Bool) : Nat :=
  if numRuns l = 0 ∧ l ≠ [] ∧ l[0]! = true then 1 else numRuns l

/-! ### Pure helper lemmas -/

theorem runsUpTo_le (l : List Bool) (k : Nat) : runsUpTo l k ≤ k := by
  unfold runsUpTo
  calc (List.range k).countP (transAt l)
      ≤ (List.range k).length := List.countP_le_length
    _ = k := List.length_range

theorem runsUpTo_succ (l : List Bool) (k : Nat) :
    runsUpTo l (k + 1) = runsUpTo l k + (if transAt l k then 1 else 0) := by
  unfold runsUpTo
  rw [List.range_succ, List.countP_append]
  simp [List.countP_cons, List.countP_nil]

/-- No new run starts at a non-transition. -/
theorem runsUpTo_succ_false (l : List Bool) (k : Nat) (h : transAt l k = false) :
    runsUpTo l (k + 1) = runsUpTo l k := by
  rw [runsUpTo_succ, h]; simp

/-- A new run starts at a transition. -/
theorem runsUpTo_succ_true (l : List Bool) (k : Nat) (h : transAt l k = true) :
    runsUpTo l (k + 1) = runsUpTo l k + 1 := by
  rw [runsUpTo_succ, h]; simp

/-! ### The loop refinement -/

/-- The lifted loop from `(runs, i)` returns the total transition count `numRuns`,
    given the forward-count precondition `runs = runsUpTo i`. -/
theorem cyclic_true_runs_loop_spec
    (sectors : Slice Bool) (n runs i : Std.Usize)
    (hn : n.val = sectors.length)
    (hb : 2 * sectors.length ≤ Std.Usize.max)
    (hi : i.val ≤ n.val)
    (hruns : runs.val = runsUpTo sectors.val i.val) :
    arrange.cyclic_true_runs_loop sectors n runs i
      ⦃ r => r.val = numRuns sectors.val ⦄ := by
  unfold arrange.cyclic_true_runs_loop
  apply loop.spec_decr_nat
    (measure := fun st => n.val - st.2.val)
    (inv := fun st => st.2.val ≤ n.val ∧ st.1.val = runsUpTo sectors.val st.2.val)
  · rintro ⟨runs', i'⟩ ⟨hle, heq⟩
    simp only [] at hle heq
    show arrange.cyclic_true_runs_loop.body sectors n runs' i' ⦃ _ ⦄
    unfold arrange.cyclic_true_runs_loop.body
    by_cases hlt : i' < n
    · -- process sector i'
      have hltN : i'.val < n.val := by scalar_tac
      have hltL : i'.val < sectors.length := by scalar_tac
      rw [if_pos hlt]
      -- b = sectors[i']
      have hib : i'.val < sectors.length := hltL
      step as ⟨b, hbv⟩
      -- the transition contribution at i'
      have hpred : ((i'.val + n.val - 1) % n.val) < sectors.length := by
        rw [hn]; exact Nat.mod_lt _ (by scalar_tac)
      -- evaluate runs1 and i1
      by_cases hbt : b = true
      · subst hbt
        simp only [if_true]
        -- predecessor index p = (i' + n - 1) % n
        have hovf : i'.val + n.val ≤ Std.Usize.max := by scalar_tac
        step as ⟨p1, hp1⟩            -- i1 = i' + n
        step as ⟨p2, hp2⟩            -- i2 = i1 - 1
        step as ⟨p3, hp3⟩            -- i3 = i2 % n
        have hp3L : p3.val < sectors.length := by
          rw [hp3, hp2, hp1, hn]; exact Nat.mod_lt _ (by scalar_tac)
        step as ⟨b1, hb1v⟩          -- b1 = sectors[i3]
        have htrans : transAt sectors.val i'.val = !b1 := by
          have e1 : sectors.val[i'.val]! = true := by
            rw [getElem!_pos sectors.val i'.val hltL]; exact hbv.symm
          have hidx : (i'.val + sectors.val.length - 1) % sectors.val.length = p3.val := by
            rw [hp3, hp2, hp1, hn]
          have e2 : sectors.val[(i'.val + sectors.val.length - 1) % sectors.val.length]! = b1 := by
            rw [hidx, getElem!_pos sectors.val p3.val hp3L]; exact hb1v.symm
          unfold transAt; rw [e1, e2]; simp
        by_cases hb1t : b1 = true
        · subst hb1t
          simp only [if_true]
          step as ⟨i1, hi1⟩          -- i' + 1
          have htf : transAt sectors.val i'.val = false := by rw [htrans]; rfl
          refine ⟨by scalar_tac, ?_, by scalar_tac⟩
          rw [hi1, runsUpTo_succ_false _ _ htf, heq]
        · have hb1f : b1 = false := by cases b1 <;> simp_all
          subst hb1f
          simp only [Bool.false_eq_true, if_false]
          have hrle : runs'.val ≤ i'.val := by
            rw [heq]; exact runsUpTo_le sectors.val i'.val
          have hrmax : runs'.val + 1 ≤ Std.Usize.max := by scalar_tac
          step as ⟨r1, hr1⟩          -- runs' + 1
          step as ⟨i1, hi1⟩          -- i' + 1
          have htt : transAt sectors.val i'.val = true := by rw [htrans]; rfl
          refine ⟨by scalar_tac, ?_, by scalar_tac⟩
          rw [hi1, runsUpTo_succ_true _ _ htt, hr1, heq]
      · have hbf : b = false := by cases b <;> simp_all
        subst hbf
        simp only [Bool.false_eq_true, if_false]
        have htf : transAt sectors.val i'.val = false := by
          have e1 : sectors.val[i'.val]! = false := by
            rw [getElem!_pos sectors.val i'.val hltL]; exact hbv.symm
          unfold transAt; rw [e1]; simp
        step as ⟨i1, hi1⟩            -- i' + 1
        refine ⟨by scalar_tac, ?_, by scalar_tac⟩
        rw [hi1, runsUpTo_succ_false _ _ htf, heq]
    · -- done: i' = n, the postcondition holds
      rw [if_neg hlt, WP.spec_ok]
      have hin : i'.val = n.val := by scalar_tac
      simp only [numRuns]; rw [heq, hin, hn]
  · exact ⟨hi, hruns⟩


/-! ### `all_true` totality (for chaining through `classify_link`'s interior case) -/

/-- The lifted `all_true` always succeeds (a total early-exit scan). We only need its
    totality to chain through `classify_link` — `link_ok` does not depend on its value. -/
theorem all_true_total (sectors : Slice Bool) :
    arrange.all_true sectors ⦃ fun (_ : Bool) => True ⦄ := by
  unfold arrange.all_true arrange.all_true_loop
  apply loop.spec_decr_nat
    (measure := fun (i : Std.Usize) => sectors.length - i.val)
    (inv := fun (i : Std.Usize) => i.val ≤ sectors.length)
  · intro i' hle
    have hle : i'.val ≤ sectors.length := hle
    show arrange.all_true_loop.body sectors i' ⦃ _ ⦄
    unfold arrange.all_true_loop.body
    by_cases hlt : i' < Slice.len sectors
    · have hltL : i'.val < sectors.length := by scalar_tac
      rw [if_pos hlt]
      step as ⟨b, _⟩
      by_cases hbt : b = true
      · subst hbt; simp only [if_true]
        step as ⟨i1, hi1⟩
        exact ⟨by scalar_tac, by scalar_tac⟩
      · have hbf : b = false := by cases b <;> simp_all
        subst hbf; simp
    · rw [if_neg hlt]; simp
  · scalar_tac

/-! ### The `cyclic_true_runs` spec -/

/-- The lifted `cyclic_true_runs` computes the cyclic-run count `cyclicRuns`. -/
theorem cyclic_true_runs_spec (sectors : Slice Bool)
    (hb : 2 * sectors.length ≤ Std.Usize.max) :
    arrange.cyclic_true_runs sectors ⦃ r => r.val = cyclicRuns sectors.val ⦄ := by
  unfold arrange.cyclic_true_runs
  by_cases hn0 : sectors.length = 0
  · -- empty: cyclicRuns = numRuns = 0
    have hlz : Slice.len sectors = 0#usize := by scalar_tac
    rw [hlz]; simp only [reduceIte, WP.spec_ok]
    have hl : sectors.val = [] := List.length_eq_zero_iff.mp hn0
    simp [cyclicRuns, numRuns, runsUpTo, hl]
  · -- nonempty: loop = numRuns, then the all-true fixup
    have hlen : (Slice.len sectors).val = sectors.length := Slice.len_val _
    have hne : Slice.len sectors ≠ 0#usize := by scalar_tac
    rw [if_neg hne]
    have hpos : 0 < sectors.length := Nat.pos_of_ne_zero hn0
    step with cyclic_true_runs_loop_spec sectors (Slice.len sectors) 0#usize 0#usize
      hlen hb (by scalar_tac) (by simp [runsUpTo]) as ⟨runs, hrunsv⟩
    by_cases hz : runs = 0#usize
    · subst hz
      simp only [reduceIte]
      have hnum0 : numRuns sectors.val = 0 := by simpa using hrunsv.symm
      step as ⟨b0, hb0⟩
      have hget : sectors.val[0]! = b0 := by
        rw [show (0 : ℕ) = (0#usize).val from rfl,
            getElem!_pos sectors.val (0#usize).val (by scalar_tac)]
        exact hb0.symm
      by_cases hb0t : b0 = true
      · subst hb0t; simp only [if_true]; rw [WP.spec_ok]
        have hne2 : sectors.val ≠ [] := fun h => hn0 (by simp [h])
        have hcr : cyclicRuns sectors.val = 1 := by
          have hcond : numRuns sectors.val = 0 ∧ sectors.val ≠ [] ∧ sectors.val[0]! = true :=
            ⟨hnum0, hne2, hget⟩
          unfold cyclicRuns; exact if_pos hcond
        simp [hcr]
      · have hb0f : b0 = false := by cases b0 <;> simp_all
        subst hb0f; simp only [Bool.false_eq_true, if_false]; rw [WP.spec_ok]
        have hcr : cyclicRuns sectors.val = 0 := by
          have hnc : ¬(numRuns sectors.val = 0 ∧ sectors.val ≠ [] ∧ sectors.val[0]! = true) := by
            rintro ⟨_, _, h⟩; rw [hget] at h; simp at h
          unfold cyclicRuns; exact (if_neg hnc).trans hnum0
        simp [hcr]
    · rw [if_neg hz, WP.spec_ok]
      have hnpos : numRuns sectors.val ≠ 0 := by
        rw [← hrunsv]; intro h; apply hz; scalar_tac
      simp only [cyclicRuns]; rw [if_neg (by tauto), hrunsv]

/-! ### The CAP-OUT-LINK spec, deductively -/

/-- **CAP-OUT-LINK, deductively:** the lifted `link_ok` accepts iff the cyclic-run count
    is `≤ 1` — the vertex is a manifold interval, not a pinch (spec §8.5). The Lean
    analogue of the bounded Kani `link_ok_iff_no_pinch`, over the Aeneas-lifted model. -/
theorem link_ok_spec (sectors : Slice Bool)
    (hb : 2 * sectors.length ≤ Std.Usize.max) :
    arrange.link_ok sectors ⦃ r => r = decide (cyclicRuns sectors.val ≤ 1) ⦄ := by
  unfold arrange.link_ok arrange.classify_link
  step with cyclic_true_runs_spec sectors hb as ⟨runs, hrunsv⟩
  rcases (show runs.val = 0 ∨ runs.val = 1 ∨ 2 ≤ runs.val by omega) with h | h | h
  · rw [h]; simp [show cyclicRuns sectors.val = 0 by omega]
  · rw [h]
    step with all_true_total sectors as ⟨ab⟩
    rcases ab with _ | _ <;> simp [show cyclicRuns sectors.val = 1 by omega]
  · obtain ⟨k, hk⟩ : ∃ k, runs.val = k + 2 := ⟨runs.val - 2, by omega⟩
    rw [hk]; simp [show cyclicRuns sectors.val = k + 2 by omega]

end CertifyCheck
