/-
  Phase 1, Aeneas route — the lifted-model refinement proof (closing report §5).

  `lattice.sturm.sign_variations` (the Aeneas-lifted `Result`-monadic loop over the
  slice iterator) computes the mathematical `signVariations` of the slice. Proven
  the *intended* Aeneas way — `loop.spec_decr_nat` for the loop + the `step` tactic
  for the body — after supplying the one `@[step]` spec Aeneas's library is missing:
  the shared-slice iterator's `next` (it ships one for `RangeIter`/`StepBy`, not for
  `IteratorSliceIter`). This mirrors `core.iter.range.IteratorRange.next_'S_spec`.
-/
import Lattice.Funs
import CertifyCheck.SignVariations
import CertifyCheck.LiftedAeneas

open Aeneas Aeneas.Std

namespace CertifyCheck

/-- Sign-change count is bounded by the length (each element adds ≤ 1) — discharges
    the `U32` overflow of `v += 1`. -/
theorem svAux_le_length (last : Int) (l : List Int) : svAux last l ≤ l.length := by
  induction l generalizing last with
  | nil => simp [svAux]
  | cons s rest ih =>
      rw [svAux]; simp only [List.length_cons]
      split
      · exact le_trans (ih last) (Nat.le_succ _)
      · have := ih s; split <;> omega

theorem sliceInts_length (signs : Slice Std.I8) :
    (sliceInts signs).length = signs.length := by
  simp [sliceInts, Slice.length]

/-- **The missing `@[step]` spec** for the shared-slice iterator's `next`
    (analogue of `core.iter.range.IteratorRange.next_'S_spec`): conditional
    postcondition — yields `some slice[i]` and advances if in range, else `none`. -/
@[step]
theorem sliceIter_next_spec {T : Type} [Inhabited T] (it : core.slice.iter.Iter T) :
    core.slice.iter.IteratorSliceIter.next it
    ⦃ (opt : Option T) (it' : core.slice.iter.Iter T) =>
      (if it.i < it.slice.val.length then
         opt = some it.slice.val[it.i]! ∧ it'.i = it.i + 1
       else opt = none ∧ it'.i = it.i) ∧ it'.slice = it.slice ⦄ := by
  rw [core.slice.iter.IteratorSliceIter.next]
  split
  · rename_i h
    have hlen : it.i < it.slice.val.length := by scalar_tac
    simp only [WP.spec_ok, WP.uncurry', hlen, ↓reduceIte, and_true,
      getElem!_pos it.slice.val it.i hlen]
    rfl
  · rename_i h
    have hge : ¬ it.i < it.slice.val.length := by scalar_tac
    simp only [WP.spec_ok, WP.uncurry', hge, ↓reduceIte, and_true]

end CertifyCheck

namespace CertifyCheck

/-- **The refinement.** The Aeneas-lifted `sign_variations` succeeds and returns the
    mathematical `signVariations` of the slice, given the length fits `U32`. -/
theorem sign_variations_spec (signs : Slice Std.I8)
    (hlen : signs.length ≤ Std.U32.max) :
    lattice.sturm.sign_variations signs
      ⦃ r => r.val = signVariations (sliceInts signs) ⦄ := by
  have htgt : signVariations (sliceInts signs) = svAux 0 (sliceInts signs) := by
    rw [← signVariationsImp_eq_signVariations]; rfl
  have hbound : svAux 0 (sliceInts signs) ≤ Std.U32.max :=
    le_trans (svAux_le_length 0 _) (by rw [sliceInts_length]; exact hlen)
  rw [lifted_sign_variations_eq_loop]
  unfold lattice.sturm.sign_variations_loop
  set I := sliceInts signs with hIdef
  clear_value I
  apply loop.spec_decr_nat
    (measure := fun st => I.length - st.1.i)
    (inv := fun st => st.1.slice = signs ∧ st.1.i ≤ I.length ∧
             (st.2.2).val + svAux (st.2.1).val (I.drop st.1.i) = svAux 0 I)
  · rintro ⟨it, last, v⟩ ⟨hslice, hle, heq⟩
    simp only [] at hslice hle heq
    show lattice.sturm.sign_variations_loop.body it last v ⦃ _ ⦄
    unfold lattice.sturm.sign_variations_loop.body
    step as ⟨opt, it', hnext, hsleq⟩
    have hlenI : it.slice.val.length = I.length := by
      rw [hslice, hIdef]; exact (sliceInts_length signs).symm
    by_cases hlt : it.i < it.slice.val.length
    · -- consumed an element `s`
      rw [if_pos hlt] at hnext
      obtain ⟨hopt, hi'⟩ := hnext
      rw [hopt]; simp only []
      set s := it.slice.val[it.i]! with hsdef
      clear_value s
      have hltI : it.i < I.length := hlenI ▸ hlt
      have hsval : s.val = I[it.i]'hltI := by
        rw [hsdef, hslice, getElem!_pos signs.val it.i (hslice ▸ hlt)]
        simp only [hIdef, sliceInts, List.getElem_map]
      have hdrop : I.drop it.i = s.val :: I.drop it'.i := by
        rw [hi', hsval]; exact (List.getElem_cons_drop hltI).symm
      have hidec : it'.i ≤ I.length := by rw [hi']; omega
      have hmeas : I.length - it'.i < I.length - it.i := by rw [hi']; omega
      have hisl : it'.slice = signs := by rw [hsleq, hslice]
      rw [hdrop, svAux] at heq
      by_cases hs0 : s = 0#i8
      · have hsv : s.val = 0 := by rw [hs0]; scalar_tac
        rw [hs0]; simp only [bne_self_eq_false, Bool.false_eq_true, if_false]
        rw [WP.spec_ok, if_pos hsv] at *
        exact ⟨hisl, hidec, heq, hmeas⟩
      · have hsv : s.val ≠ 0 := fun h => hs0 (by scalar_tac)
        rw [bne_iff_ne.mpr hs0]; simp only [if_true]; rw [if_neg hsv] at heq
        by_cases hl0 : last = 0#i8
        · have hlv : last.val = 0 := by rw [hl0]; scalar_tac
          rw [hl0]; simp only [bne_self_eq_false, Bool.false_eq_true, if_false]
          rw [WP.spec_ok]; rw [if_neg (by simp [hlv]), zero_add] at heq
          exact ⟨hisl, hidec, heq, hmeas⟩
        · have hlv : last.val ≠ 0 := fun h => hl0 (by scalar_tac)
          rw [bne_iff_ne.mpr hl0]; simp only [if_true]
          by_cases hsl : s = last
          · have hslv : s.val = last.val := by rw [hsl]
            have hbsl : (s != last) = false := by rw [hsl]; exact bne_self_eq_false _
            rw [hbsl]; simp only [Bool.false_eq_true, if_false]
            rw [WP.spec_ok]; rw [if_neg (by simp [hslv]), zero_add] at heq
            exact ⟨hisl, hidec, heq, hmeas⟩
          · have hslv : s.val ≠ last.val := fun h => hsl (by scalar_tac)
            rw [bne_iff_ne.mpr hsl]; simp only [if_true]
            rw [if_pos ⟨hlv, hslv⟩] at heq
            have hone : (1#u32).val = 1 := by scalar_tac
            have hmax : v.val + (1#u32).val ≤ Std.U32.max := by rw [hone]; omega
            step as ⟨v1, hv1⟩
            refine ⟨hisl, hidec, ?_, hmeas⟩
            omega
    · -- iterator exhausted: done
      rw [if_neg hlt] at hnext
      obtain ⟨hopt, _⟩ := hnext
      rw [hopt]; simp only []
      rw [WP.spec_ok]
      have hge : ¬ it.i < I.length := hlenI ▸ hlt
      have hi : it.i = I.length := le_antisymm hle (Nat.le_of_not_lt hge)
      rw [hi] at heq
      simp only [List.drop_length, svAux, Nat.add_zero] at heq
      rw [htgt]; exact heq
  · refine ⟨rfl, by simp, ?_⟩; simp

end CertifyCheck
