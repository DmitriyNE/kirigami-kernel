//! Pure gate algebra.
//!
//! The reusable verdict-propagation combinator the workspace's hand-rolled 3-arm
//! conjunction matches reduce to (canonically `closure::valid::closure_valid`, one of 121
//! such matches workspace-wide), plus the spec §8.6 gate formula `VALID_solid-closure`
//! expressed as that fold. Gate formulas contain only truth-valued certificate
//! expressions — no imperatives, no "band or fail" disjunct (spec §8.2/§8.6). Pure, total,
//! panic-free `no_std`; the extraction/TCB surface. The append-only, provenance-linked,
//! FRESH-promoting certificate *store* lives in the `gate` shell crate (M6.2); this is
//! only the algebra it evaluates.

use crate::shell::{ClosedShell, ClosedShellFault};
use crate::verdict::Verdict;

/// The strong-Kleene **conjunction** of a sequence of verdicts.
///
/// A conjunction is refuted by a single false conjunct, unknown if any conjunct is
/// unknown (and none false), and verified only if every conjunct is verified. This
/// folds that rule over `verdicts`, in order:
///
/// - **all [`Verified`]** ⇒ [`Verified(())`] — the conjunction holds. The evidence is
///   unit: a gate formula is truth-valued, and the per-conjunct evidence is retained by
///   the caller (the certificate store), not re-bundled here.
/// - **any [`Refuted`]** ⇒ the **first** (leftmost) [`Refuted`], returned as soon as it
///   is reached — one false conjunct falsifies the whole conjunction regardless of
///   position, so a later verdict cannot change the outcome.
/// - **otherwise** (some [`Unresolved`], none [`Refuted`]) ⇒ the **first** (leftmost)
///   [`Unresolved`] — the honest three-valued middle: the conjunction cannot be
///   decided, and its refinement handle is that conjunct's margin.
///
/// Allocation-free and total. This is the algebra proven sound by the ★ Kani harness
/// `gate_conj_sound` (soundness *and* completeness over the three-valued lattice, plus
/// that the selected witness/margin is the leftmost of its kind).
///
/// [`Verified`]: Verdict::Verified
/// [`Refuted`]: Verdict::Refuted
/// [`Unresolved`]: Verdict::Unresolved
/// [`Verified(())`]: Verdict::Verified
///
/// # Example
///
/// ```
/// use certify_core::gate::conj;
/// use certify_core::verdict::Verdict;
///
/// // All conjuncts verified ⇒ the conjunction holds.
/// let all_ok = [Verdict::<i32, &str, ()>::Verified(1), Verdict::Verified(2)];
/// assert_eq!(conj(all_ok), Verdict::Verified(()));
///
/// // A single refuted conjunct falsifies it — the *first* refuter is returned, even
/// // past an earlier unresolved conjunct.
/// let mixed = [
///     Verdict::<i32, &str, ()>::Unresolved(()),
///     Verdict::Refuted("A"),
///     Verdict::Refuted("B"),
/// ];
/// assert_eq!(conj(mixed), Verdict::Refuted("A"));
///
/// // No refuter, but an unresolved conjunct ⇒ unresolved (the first one).
/// let unknown = [Verdict::<i32, &str, i32>::Verified(1), Verdict::Unresolved(7)];
/// assert_eq!(conj(unknown), Verdict::Unresolved(7));
/// ```
pub fn conj<E, W, M>(verdicts: impl IntoIterator<Item = Verdict<E, W, M>>) -> Verdict<(), W, M> {
    let mut first_unresolved: Option<M> = None;
    for v in verdicts {
        match v {
            Verdict::Verified(_) => {}
            // First (leftmost) refuter: return at once — no later verdict can revive
            // a refuted conjunction.
            Verdict::Refuted(w) => return Verdict::Refuted(w),
            Verdict::Unresolved(m) => {
                if first_unresolved.is_none() {
                    first_unresolved = Some(m);
                }
            }
        }
    }
    match first_unresolved {
        Some(m) => Verdict::Unresolved(m),
        None => Verdict::Verified(()),
    }
}

/// The evidence a [`Verified`](Verdict::Verified) `VALID_solid-closure` carries: how many
/// joints' `CLOSURE_VALID(j)` were certified. A gate certificate is truth-valued, so this
/// is a marker — the per-joint witnesses live in the certificate store with their
/// provenance. On the one-joint straight-crease slice this is `1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SolidClosure {
    /// Number of joints whose `CLOSURE_VALID(j)` conjunct was verified.
    pub joints_certified: usize,
}

/// Which conjunct of `VALID_solid-closure` refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolidClosureFault<W> {
    /// The `VALID_complement` conjunct refused. Vacuous on the one-joint straight-crease
    /// slice (no complement clips), so unreachable there; present so the fold is the full
    /// spec formula, not a special case. Milestone D, adding real complement clips, gives
    /// this variant a distinct witness.
    Complement,
    /// Joint `joint`'s `CLOSURE_VALID(joint)` refused, carrying its refutation.
    Closure {
        /// Index of the refusing joint into the `per_joint` sequence.
        joint: usize,
        /// The joint's `CLOSURE_VALID` witness.
        witness: W,
    },
}

/// `VALID_solid-closure` (spec §8.6): the complement is watertight **and** every joint
/// closes. Cites the spec formula `VALID_complement ∧ ⋀_j CLOSURE_VALID(j)` — evaluated as
/// a [`conj`] fold over the `VALID_complement` verdict followed by the per-joint
/// `CLOSURE_VALID(j)` verdicts, each tagged with its provenance ([`SolidClosureFault`]).
///
/// On the thin one-joint straight-crease slice `VALID_complement` is **vacuously
/// [`Verified`](Verdict::Verified)** (no complement clips), so the caller passes
/// `Verdict::Verified(())`; the fold then reduces to the single joint's verdict. The
/// [`Verified`](Verdict::Verified) evidence counts the certified joints; a
/// [`Refuted`](Verdict::Refuted) names the first failing conjunct.
///
/// # Example
///
/// ```
/// use certify_core::gate::{valid_solid_closure, SolidClosure, SolidClosureFault};
/// use certify_core::verdict::Verdict;
///
/// // A one-joint slice: complement vacuously verified, the joint certified.
/// let complement = Verdict::Verified(());
/// let joints = [Verdict::<i32, &str, ()>::Verified(0)];
/// assert_eq!(
///     valid_solid_closure(&complement, &joints),
///     Verdict::Verified(SolidClosure { joints_certified: 1 }),
/// );
///
/// // A refusing joint is named by index.
/// let joints = [Verdict::<i32, &str, ()>::Refuted("REG-V")];
/// assert_eq!(
///     valid_solid_closure(&complement, &joints),
///     Verdict::Refuted(SolidClosureFault::Closure { joint: 0, witness: "REG-V" }),
/// );
/// ```
pub fn valid_solid_closure<E, W: Clone>(
    complement: &Verdict<(), (), ()>,
    per_joint: &[Verdict<E, W, ()>],
) -> Verdict<SolidClosure, SolidClosureFault<W>, ()> {
    let joints_certified = per_joint.len();

    // Tag each conjunct's witness with its provenance, then fold with the pure `conj`. The
    // evidence `E` is dropped (a gate formula is truth-valued); only the witness is lifted.
    let complement = tag_complement(complement);
    let joints = per_joint
        .iter()
        .enumerate()
        .map(|(joint, v)| tag_joint(joint, v));

    match conj(core::iter::once(complement).chain(joints)) {
        Verdict::Verified(()) => Verdict::Verified(SolidClosure { joints_certified }),
        Verdict::Refuted(fault) => Verdict::Refuted(fault),
        Verdict::Unresolved(()) => Verdict::Unresolved(()),
    }
}

/// Lift the `VALID_complement` verdict into the tagged conjunct stream. Generic over the
/// joint witness `W` (the `Complement` variant is a unit — it carries no payload of its
/// own), so it unifies with the tagged joint conjuncts at the fold.
fn tag_complement<W>(v: &Verdict<(), (), ()>) -> Verdict<(), SolidClosureFault<W>, ()> {
    match v {
        Verdict::Verified(()) => Verdict::Verified(()),
        Verdict::Refuted(()) => Verdict::Refuted(SolidClosureFault::Complement),
        Verdict::Unresolved(()) => Verdict::Unresolved(()),
    }
}

/// Lift joint `joint`'s `CLOSURE_VALID` verdict into the tagged conjunct stream, cloning
/// only the witness (the evidence `E` is dropped — the fold is truth-valued).
fn tag_joint<E, W: Clone>(
    joint: usize,
    v: &Verdict<E, W, ()>,
) -> Verdict<(), SolidClosureFault<W>, ()> {
    match v {
        Verdict::Verified(_) => Verdict::Verified(()),
        Verdict::Refuted(w) => Verdict::Refuted(SolidClosureFault::Closure {
            joint,
            witness: w.clone(),
        }),
        Verdict::Unresolved(()) => Verdict::Unresolved(()),
    }
}

/// The evidence a [`Verified`](Verdict::Verified) `valid_closed_solid` carries: the joints
/// certified plus the assembled shell's element counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClosedSolid {
    /// Number of joints whose `CLOSURE_VALID(j)` conjunct was verified (from `SolidClosure`).
    pub joints_certified: usize,
    /// Vertices of the certified closed shell.
    pub verts: usize,
    /// Edges of the certified closed shell.
    pub edges: usize,
    /// Faces of the certified closed shell.
    pub faces: usize,
    /// Boundary loops of the certified closed shell (`≥ faces`; the excess counts holes).
    pub loops: usize,
}

/// Which conjunct of `valid_closed_solid` refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosedSolidFault<W> {
    /// The `VALID_solid-closure` conjunct refused (a joint or the complement), carrying its
    /// [`SolidClosureFault`].
    SolidClosure(SolidClosureFault<W>),
    /// The whole-solid `closed_shell` conjunct refused, carrying its [`ClosedShellFault`].
    Shell(ClosedShellFault),
}

/// `valid_closed_solid` — the atlas-assembly gate (Milestone D slice 4): the joints close
/// **and** the assembled shell is a certified closed 2-manifold. This **extends beyond** the
/// spec §8.6 formula (`VALID_solid-closure` is joint-local, `spec:439`); the whole-solid
/// closedness conjunct is the layer the docs pre-name ("ruled sidewalls carrying their own
/// CAP-OUT/SEW-LINK coverage → whole-solid watertightness certified", `vv-guide.md:1102`) and
/// the assembly-scale analogue of the `CapOut.lean:25-30` frontier — proven internally by
/// [`closed_shell`](crate::shell::closed_shell), never delegated to an external kernel.
///
/// A strong-Kleene conjunction of the existing [`valid_solid_closure`] verdict (the leftmost
/// conjunct) and the [`closed_shell`](crate::shell::closed_shell) verdict, preserving both
/// evidences: [`Verified`](Verdict::Verified) with the combined [`ClosedSolid`], or
/// [`Refuted`](Verdict::Refuted) naming the first failing conjunct (a `Refuted` conjunct
/// dominates an `Unresolved` one, per the [`conj`] rule).
///
/// # Example
///
/// ```
/// use certify_core::gate::{valid_closed_solid, ClosedSolid, ClosedSolidFault, SolidClosure};
/// use certify_core::shell::{ClosedShell, ClosedShellFault};
/// use certify_core::verdict::Verdict;
///
/// use certify_core::gate::SolidClosureFault;
///
/// // A one-joint slice that closes, whose assembled shell is a certified closed cube.
/// // (the joint-witness type `&str` fixes the fault channel `W`.)
/// let solid_closure: Verdict<SolidClosure, SolidClosureFault<&str>, ()> =
///     Verdict::Verified(SolidClosure { joints_certified: 1 });
/// let shell = Verdict::Verified(ClosedShell { verts: 8, edges: 12, faces: 6, loops: 6 });
/// assert_eq!(
///     valid_closed_solid(&solid_closure, &shell),
///     Verdict::Verified(ClosedSolid { joints_certified: 1, verts: 8, edges: 12, faces: 6, loops: 6 }),
/// );
///
/// // An open shell refuses even when the joints close.
/// let bad_shell = Verdict::Refuted(ClosedShellFault::EdgeCensus { edge: 3 });
/// assert_eq!(
///     valid_closed_solid(&solid_closure, &bad_shell),
///     Verdict::Refuted(ClosedSolidFault::Shell(ClosedShellFault::EdgeCensus { edge: 3 })),
/// );
/// ```
pub fn valid_closed_solid<W: Clone>(
    solid_closure: &Verdict<SolidClosure, SolidClosureFault<W>, ()>,
    shell: &Verdict<ClosedShell, ClosedShellFault, ()>,
) -> Verdict<ClosedSolid, ClosedSolidFault<W>, ()> {
    match solid_closure {
        // The leftmost conjunct: a refutation here dominates whatever the shell says.
        Verdict::Refuted(f) => Verdict::Refuted(ClosedSolidFault::SolidClosure(f.clone())),
        // Solid-closure unresolved: a shell refutation still dominates (strong-Kleene);
        // otherwise the conjunction is unresolved.
        Verdict::Unresolved(()) => match shell {
            Verdict::Refuted(sf) => Verdict::Refuted(ClosedSolidFault::Shell(*sf)),
            _ => Verdict::Unresolved(()),
        },
        // Solid-closure holds: the shell verdict carries the conjunction.
        Verdict::Verified(sc) => match shell {
            Verdict::Verified(sh) => Verdict::Verified(ClosedSolid {
                joints_certified: sc.joints_certified,
                verts: sh.verts,
                edges: sh.edges,
                faces: sh.faces,
                loops: sh.loops,
            }),
            Verdict::Refuted(sf) => Verdict::Refuted(ClosedSolidFault::Shell(*sf)),
            Verdict::Unresolved(()) => Verdict::Unresolved(()),
        },
    }
}

// The doctests on `conj` / `valid_solid_closure` above are the usage examples; these unit
// tests pin the ordering corners the ★ `gate_conj_sound` harness proves in general.
#[cfg(test)]
mod tests {
    use super::*;

    type V = Verdict<i32, &'static str, i32>;

    #[test]
    fn empty_conjunction_is_vacuously_verified() {
        let none: [V; 0] = [];
        assert_eq!(conj(none), Verdict::Verified(()));
    }

    #[test]
    fn a_refuter_dominates_an_earlier_unresolved() {
        // The non-trivial rule: Refuted wins even when an Unresolved precedes it, and the
        // *first* refuter is the one returned.
        let vs = [
            V::Unresolved(1),
            V::Refuted("first"),
            V::Unresolved(2),
            V::Refuted("second"),
        ];
        assert_eq!(conj(vs), Verdict::Refuted("first"));
    }

    #[test]
    fn first_unresolved_when_no_refuter() {
        let vs = [V::Verified(0), V::Unresolved(7), V::Unresolved(9)];
        assert_eq!(conj(vs), Verdict::Unresolved(7));
    }

    #[test]
    fn solid_closure_folds_many_joints() {
        let complement = Verdict::Verified(());
        // Three joints, all certified ⇒ the whole solid closure is verified.
        let joints = [
            Verdict::<i32, &str, ()>::Verified(0),
            Verdict::Verified(1),
            Verdict::Verified(2),
        ];
        assert_eq!(
            valid_solid_closure(&complement, &joints),
            Verdict::Verified(SolidClosure {
                joints_certified: 3
            }),
        );

        // The middle joint refuses ⇒ named by index, past the verified first joint.
        let joints = [
            Verdict::<i32, &str, ()>::Verified(0),
            Verdict::Refuted("CLIP-DOM"),
            Verdict::Verified(2),
        ];
        assert_eq!(
            valid_solid_closure(&complement, &joints),
            Verdict::Refuted(SolidClosureFault::Closure {
                joint: 1,
                witness: "CLIP-DOM",
            }),
        );
    }

    #[test]
    fn a_refused_complement_is_named() {
        // VALID_complement is the first conjunct: if it refuses (Milestone D regime), the
        // fold names it — and it dominates a later unresolved joint.
        let complement = Verdict::Refuted(());
        let joints = [Verdict::<i32, &str, ()>::Unresolved(())];
        assert_eq!(
            valid_solid_closure(&complement, &joints),
            Verdict::Refuted(SolidClosureFault::Complement),
        );
    }

    #[test]
    fn an_unresolved_joint_is_unresolved() {
        let complement = Verdict::Verified(());
        let joints = [
            Verdict::<i32, &str, ()>::Verified(0),
            Verdict::Unresolved(()),
        ];
        assert_eq!(
            valid_solid_closure(&complement, &joints),
            Verdict::Unresolved(()),
        );
    }

    #[test]
    fn closed_solid_conjoins_closure_and_shell() {
        use crate::shell::{ClosedShell, ClosedShellFault};

        let sc: Verdict<SolidClosure, SolidClosureFault<&str>, ()> =
            Verdict::Verified(SolidClosure {
                joints_certified: 1,
            });
        let sh = Verdict::Verified(ClosedShell {
            verts: 8,
            edges: 12,
            faces: 6,
            loops: 6,
        });
        // Both hold ⇒ the combined evidence.
        assert_eq!(
            valid_closed_solid(&sc, &sh),
            Verdict::Verified(ClosedSolid {
                joints_certified: 1,
                verts: 8,
                edges: 12,
                faces: 6,
                loops: 6,
            }),
        );

        // A refused solid-closure is the *leftmost* conjunct: it dominates even a refused shell.
        let sc_bad: Verdict<SolidClosure, SolidClosureFault<&str>, ()> =
            Verdict::Refuted(SolidClosureFault::Closure {
                joint: 0,
                witness: "REG-V",
            });
        let sh_bad =
            Verdict::<ClosedShell, ClosedShellFault, ()>::Refuted(ClosedShellFault::VertexLink {
                vertex: 2,
            });
        assert_eq!(
            valid_closed_solid(&sc_bad, &sh_bad),
            Verdict::Refuted(ClosedSolidFault::SolidClosure(SolidClosureFault::Closure {
                joint: 0,
                witness: "REG-V",
            })),
        );

        // Closure holds but the shell is open ⇒ the shell fault surfaces.
        assert_eq!(
            valid_closed_solid(&sc, &sh_bad),
            Verdict::Refuted(ClosedSolidFault::Shell(ClosedShellFault::VertexLink {
                vertex: 2
            })),
        );
    }
}
