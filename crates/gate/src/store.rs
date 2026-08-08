//! The append-only, provenance-linked certificate store.
//!
//! A [`CertStore`] is the shell-tier audit ledger the gate evaluates over. Each
//! entry is one certificate: a stable [`CertId`], a truth-valued [`Verdict`]
//! (its refutation carrying a durable descriptor), an optional **stamp** (the
//! certified enclosure, a [`MarginSq`]), and provenance links to the source
//! certificates it was derived from. The store is *append-only* — records are
//! only ever pushed, never mutated or removed, so a [`CertId`] handed out once
//! stays valid and points at the same record forever.
//!
//! [`evaluate_solid_closure`](CertStore::evaluate_solid_closure) folds the
//! stored per-joint `CLOSURE_VALID(j)` records into `VALID_solid-closure`
//! (spec §8.6) via the pure [`certify_core::gate`] algebra, and appends the
//! resulting gate certificate — provenance-linked back to the joints, stamped
//! with the conjunction's enclosure.
//!
//! # The provenance chain rule (spec:203)
//!
//! Every derived certificate's stamp is *bounded below by* the certified
//! enclosures of its sources: a conjunction is only as separated as its weakest
//! conjunct, so a derived stamp may never claim a **tighter** enclosure (a
//! larger separation margin) than the minimum over its sources.
//! [`append_derived`](CertStore::append_derived) enforces this — a stamp that
//! over-claims is rejected with [`StoreError::ChainViolation`]. The store thus
//! only ever ingests certified [`Verdict`] / [`MarginSq`] data; a naked float
//! cannot enter (there is nowhere to put one).
//!
//! # FRESH is deferred (Milestone E)
//!
//! [`fresh_recheck`](CertStore::fresh_recheck) is a documented stub. The
//! three-way containment re-test (regenerate the enclosure and test it against
//! the stored stamp: ⊆ ⇒ green, disjoint ⇒ stale/refuted, partial ⇒ unresolved)
//! keys on fab-gating fields (`materialStripWidth`), a `VALID_material` concern,
//! and is material-grade work. The chain-rule enforcement here is FRESH's
//! *precondition* and is built; the re-test itself is not.
//!
//! ```
//! use gate::store::CertStore;
//! use certify_core::verdict::Verdict;
//! use certify_core::margin::MarginSq;
//!
//! let mut store = CertStore::<i64>::new();
//!
//! // A checker's leaf certificate: joint 0's CLOSURE_VALID, Verified with a cleared
//! // squared-separation margin (never a float — only certified `MarginSq` data enters).
//! let j0 = store.append_leaf(
//!     "CLOSURE_VALID(0)".to_string(),
//!     Verdict::Verified(()),
//!     Some(MarginSq(7)),
//! );
//!
//! // VALID_complement is vacuously Verified on the one-joint straight-crease slice.
//! let complement = Verdict::Verified(());
//! let (gate_id, outcome) = store.evaluate_solid_closure(&complement, &[j0]).unwrap();
//!
//! assert!(matches!(outcome, Verdict::Verified(_)));
//! // The gate certificate is stored, provenance-linked back to the joint, and its
//! // stamp is the conjunction enclosure — the weakest source margin.
//! assert_eq!(store.get(gate_id).unwrap().provenance, vec![j0]);
//! assert_eq!(store.get(gate_id).unwrap().stamp, Some(MarginSq(7)));
//! ```

use certify_core::gate::{SolidClosure, SolidClosureFault, valid_solid_closure};
use certify_core::margin::MarginSq;
use certify_core::verdict::Verdict;

/// A stable handle to a stored certificate.
///
/// A `CertId` is the record's index at the moment it was appended. Because the
/// store is append-only, that index never shifts and never gets reused, so a
/// `CertId` is a permanent reference to one record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CertId(
    /// The record's append index — stable and never reused.
    pub usize,
);

/// The typed outcome of a `VALID_solid-closure` evaluation: [`Verified`] with
/// the certified-joint count, [`Refuted`] naming the first failing conjunct, or
/// [`Unresolved`].
///
/// [`Verified`]: Verdict::Verified
/// [`Refuted`]: Verdict::Refuted
/// [`Unresolved`]: Verdict::Unresolved
pub type SolidClosureOutcome = Verdict<SolidClosure, SolidClosureFault<String>, ()>;

/// One certificate in the store.
///
/// The [`verdict`](CertRecord::verdict) is truth-valued (evidence lives as the
/// [`stamp`](CertRecord::stamp)); a refutation carries a durable human-readable
/// descriptor rather than the transient typed witness — the store is a durable
/// audit ledger over heterogeneous certificate species, and the live typed
/// witness stays with the checker that produced it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CertRecord<T> {
    /// This record's stable handle (its append index).
    pub id: CertId,
    /// The certificate's name, e.g. `"CLOSURE_VALID(0)"` or `"VALID_solid-closure"`.
    pub label: String,
    /// The truth-valued outcome; a [`Refuted`](Verdict::Refuted) carries a descriptor.
    pub verdict: Verdict<(), String, ()>,
    /// The certified enclosure (squared separation margin). `Some` when the
    /// certificate carries one (a [`Verified`](Verdict::Verified) margin cert),
    /// `None` for a purely combinatorial certificate.
    pub stamp: Option<MarginSq<T>>,
    /// The source certificates this one was derived from (empty for a leaf).
    pub provenance: Vec<CertId>,
}

/// A rejected append.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoreError {
    /// A provenance link referenced a [`CertId`] that is not in the store.
    DanglingProvenance(CertId),
    /// The provenance chain rule (spec:203) was violated: a derived stamp
    /// claimed a tighter enclosure (a larger separation margin) than the named
    /// source justifies. A conjunction cannot be more separated than its
    /// weakest conjunct.
    ChainViolation {
        /// The source whose enclosure the derived stamp exceeded.
        source: CertId,
    },
}

/// An append-only, provenance-linked certificate store.
///
/// Generic over the margin scalar `T` (the `lattice` rational/integer the
/// stamps are drawn from); the chain-rule and conjunction-enclosure logic needs
/// only `T: Ord + Clone`.
#[derive(Clone, Debug, Default)]
pub struct CertStore<T> {
    records: Vec<CertRecord<T>>,
}

impl<T> CertStore<T> {
    /// A fresh, empty store.
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// The number of certificates stored.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the store holds no certificates.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The record at `id`, or `None` if `id` is not in the store.
    pub fn get(&self, id: CertId) -> Option<&CertRecord<T>> {
        self.records.get(id.0)
    }

    /// A read-only view of every stored record, in append order.
    pub fn records(&self) -> &[CertRecord<T>] {
        &self.records
    }

    /// Append a **leaf** certificate — a checker's direct output, with no
    /// sources. Returns its stable [`CertId`].
    pub fn append_leaf(
        &mut self,
        label: String,
        verdict: Verdict<(), String, ()>,
        stamp: Option<MarginSq<T>>,
    ) -> CertId {
        let id = CertId(self.records.len());
        self.records.push(CertRecord {
            id,
            label,
            verdict,
            stamp,
            provenance: Vec::new(),
        });
        id
    }
}

impl<T: Ord + Clone> CertStore<T> {
    /// The **weakest** source over `provenance` — the `(id, margin)` of the
    /// source carrying the smallest stamp — or `None` if none of the (existing)
    /// sources carries a stamp. This is the conjunction's certified enclosure:
    /// a conjunction is only as separated as its weakest conjunct. On a tie the
    /// first (leftmost) minimum is returned.
    fn weakest_source(&self, provenance: &[CertId]) -> Option<(CertId, MarginSq<T>)> {
        let mut weakest: Option<(CertId, MarginSq<T>)> = None;
        for &id in provenance {
            if let Some(MarginSq(m)) = self.get(id).and_then(|r| r.stamp.clone()) {
                match &weakest {
                    Some((_, MarginSq(best))) if *best <= m => {}
                    _ => weakest = Some((id, MarginSq(m))),
                }
            }
        }
        weakest
    }

    /// Append a **derived** certificate, enforcing the provenance chain rule.
    ///
    /// Every id in `provenance` must already be in the store (else
    /// [`StoreError::DanglingProvenance`]). If `stamp` is `Some`, it must not
    /// exceed the weakest source's stamp — a derived enclosure may not be
    /// tighter than its sources justify (else [`StoreError::ChainViolation`]).
    /// On success returns the new [`CertId`].
    pub fn append_derived(
        &mut self,
        label: String,
        verdict: Verdict<(), String, ()>,
        stamp: Option<MarginSq<T>>,
        provenance: Vec<CertId>,
    ) -> Result<CertId, StoreError> {
        // Every source must exist — a dangling link is a broken provenance chain.
        for &p in &provenance {
            if self.get(p).is_none() {
                return Err(StoreError::DanglingProvenance(p));
            }
        }
        // The chain rule: a derived stamp may not over-claim vs its weakest source.
        if let Some(MarginSq(derived)) = &stamp {
            if let Some((source, MarginSq(bound))) = self.weakest_source(&provenance) {
                if *derived > bound {
                    return Err(StoreError::ChainViolation { source });
                }
            }
        }
        let id = CertId(self.records.len());
        self.records.push(CertRecord {
            id,
            label,
            verdict,
            stamp,
            provenance,
        });
        Ok(id)
    }

    /// Evaluate `VALID_solid-closure` (spec §8.6) over the stored per-joint
    /// `CLOSURE_VALID(j)` records via the pure [`certify_core::gate`] algebra,
    /// and append the resulting gate certificate.
    ///
    /// `complement` is the `VALID_complement` verdict (vacuously
    /// [`Verified`](Verdict::Verified) on the one-joint straight-crease slice,
    /// where there are no complement clips); `joint_ids` are the stored joint
    /// certificates, in order. Returns the appended gate certificate's
    /// [`CertId`] and the typed [`valid_solid_closure`] outcome. The gate
    /// certificate is provenance-linked to `joint_ids` and, when
    /// [`Verified`](Verdict::Verified), stamped with the conjunction enclosure
    /// (the weakest joint's margin) — so it satisfies the chain rule by
    /// construction. A bad `joint_ids` entry yields
    /// [`StoreError::DanglingProvenance`].
    pub fn evaluate_solid_closure(
        &mut self,
        complement: &Verdict<(), (), ()>,
        joint_ids: &[CertId],
    ) -> Result<(CertId, SolidClosureOutcome), StoreError> {
        // Reconstruct the per-joint verdicts from their records (truth-valued, E = ()).
        let mut per_joint: Vec<Verdict<(), String, ()>> = Vec::with_capacity(joint_ids.len());
        for &id in joint_ids {
            let rec = self.get(id).ok_or(StoreError::DanglingProvenance(id))?;
            per_joint.push(rec.verdict.clone());
        }

        // Fold via the M6.1 pure algebra: VALID_complement ∧ ⋀_j CLOSURE_VALID(j).
        let outcome = valid_solid_closure(complement, &per_joint);

        // The derived stamp is the conjunction enclosure — the weakest source margin —
        // and only when the whole conjunction is Verified.
        let stamp = match &outcome {
            Verdict::Verified(_) => self.weakest_source(joint_ids).map(|(_, m)| m),
            _ => None,
        };

        // Record the outcome as a durable descriptor (the transient typed witness stays
        // with the checker; the ledger keeps an auditable name).
        let recorded = match &outcome {
            Verdict::Verified(_) => Verdict::Verified(()),
            Verdict::Refuted(fault) => Verdict::Refuted(render_fault(fault)),
            Verdict::Unresolved(()) => Verdict::Unresolved(()),
        };

        // The stamp equals the weakest source by construction, so the chain rule holds;
        // `append_derived` re-checks it (defence in depth) and links the provenance.
        let id = self.append_derived(
            "VALID_solid-closure".to_string(),
            recorded,
            stamp,
            joint_ids.to_vec(),
        )?;
        Ok((id, outcome))
    }

    /// FRESH re-test — **deferred to Milestone E** (material grade).
    ///
    /// The real re-test regenerates the certificate's enclosure and tests it
    /// against the stored [`stamp`](CertRecord::stamp): contained ⇒ still fresh,
    /// disjoint ⇒ stale (a refutation), partial ⇒ unresolved. It keys on
    /// fab-gating fields (`materialStripWidth`), a `VALID_material` concern, so
    /// it lands with material grade, not solid-closure. Until then this returns
    /// the honest three-valued middle unconditionally.
    pub fn fresh_recheck(&self, id: CertId) -> Verdict<(), String, ()> {
        // A real implementation would reload this record's stamp and re-test containment.
        let _stamp = self.get(id).and_then(|r| r.stamp.as_ref());
        Verdict::Unresolved(())
    }
}

/// Render a `VALID_solid-closure` fault into a durable ledger descriptor.
fn render_fault(fault: &SolidClosureFault<String>) -> String {
    match fault {
        SolidClosureFault::Complement => "VALID_complement refused".to_string(),
        SolidClosureFault::Closure { joint, witness } => {
            format!("CLOSURE_VALID({joint}) refused: {witness}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_stable_and_append_order() {
        let mut store = CertStore::<i64>::new();
        let a = store.append_leaf("A".to_string(), Verdict::Verified(()), Some(MarginSq(5)));
        let b = store.append_leaf("B".to_string(), Verdict::Verified(()), None);
        assert_eq!(a, CertId(0));
        assert_eq!(b, CertId(1));
        assert_eq!(store.len(), 2);
        assert_eq!(store.get(a).unwrap().label, "A");
        assert_eq!(store.get(b).unwrap().stamp, None);
        assert_eq!(store.get(CertId(2)), None);
    }

    #[test]
    fn one_joint_solid_closure_is_verified_and_stamped() {
        let mut store = CertStore::<i64>::new();
        let j0 = store.append_leaf(
            "CLOSURE_VALID(0)".to_string(),
            Verdict::Verified(()),
            Some(MarginSq(9)),
        );
        let complement = Verdict::Verified(());
        let (gate_id, outcome) = store.evaluate_solid_closure(&complement, &[j0]).unwrap();
        assert_eq!(
            outcome,
            Verdict::Verified(SolidClosure {
                joints_certified: 1
            })
        );
        let rec = store.get(gate_id).unwrap();
        assert_eq!(rec.label, "VALID_solid-closure");
        assert_eq!(rec.provenance, vec![j0]);
        // Conjunction enclosure = the (only) joint's margin.
        assert_eq!(rec.stamp, Some(MarginSq(9)));
    }

    #[test]
    fn conjunction_enclosure_is_the_weakest_joint() {
        let mut store = CertStore::<i64>::new();
        let j0 = store.append_leaf("C0".to_string(), Verdict::Verified(()), Some(MarginSq(9)));
        let j1 = store.append_leaf("C1".to_string(), Verdict::Verified(()), Some(MarginSq(3)));
        let j2 = store.append_leaf("C2".to_string(), Verdict::Verified(()), Some(MarginSq(7)));
        let complement = Verdict::Verified(());
        let (gate_id, _) = store
            .evaluate_solid_closure(&complement, &[j0, j1, j2])
            .unwrap();
        // The whole solid closure is only as separated as the weakest joint (3).
        assert_eq!(store.get(gate_id).unwrap().stamp, Some(MarginSq(3)));
    }

    #[test]
    fn a_refusing_joint_is_named_in_the_ledger() {
        let mut store = CertStore::<i64>::new();
        let j0 = store.append_leaf("C0".to_string(), Verdict::Verified(()), Some(MarginSq(9)));
        let j1 = store.append_leaf(
            "C1".to_string(),
            Verdict::Refuted("REG-V".to_string()),
            None,
        );
        let complement = Verdict::Verified(());
        let (gate_id, outcome) = store
            .evaluate_solid_closure(&complement, &[j0, j1])
            .unwrap();
        assert_eq!(
            outcome,
            Verdict::Refuted(SolidClosureFault::Closure {
                joint: 1,
                witness: "REG-V".to_string(),
            })
        );
        let rec = store.get(gate_id).unwrap();
        assert_eq!(
            rec.verdict,
            Verdict::Refuted("CLOSURE_VALID(1) refused: REG-V".to_string())
        );
        // A refuted conjunction carries no separation stamp.
        assert_eq!(rec.stamp, None);
    }

    #[test]
    fn dangling_provenance_is_rejected() {
        let mut store = CertStore::<i64>::new();
        let err = store
            .append_derived(
                "derived".to_string(),
                Verdict::Verified(()),
                None,
                vec![CertId(99)],
            )
            .unwrap_err();
        assert_eq!(err, StoreError::DanglingProvenance(CertId(99)));
    }

    #[test]
    fn over_claiming_stamp_violates_the_chain_rule() {
        let mut store = CertStore::<i64>::new();
        let src = store.append_leaf("src".to_string(), Verdict::Verified(()), Some(MarginSq(4)));
        // A derived stamp of 5 claims a tighter enclosure than the source's 4 — refused.
        let err = store
            .append_derived(
                "derived".to_string(),
                Verdict::Verified(()),
                Some(MarginSq(5)),
                vec![src],
            )
            .unwrap_err();
        assert_eq!(err, StoreError::ChainViolation { source: src });
        // Equalling the source (4) is allowed — bounded below by, not strictly under.
        let ok = store.append_derived(
            "derived".to_string(),
            Verdict::Verified(()),
            Some(MarginSq(4)),
            vec![src],
        );
        assert!(ok.is_ok());
    }

    #[test]
    fn fresh_recheck_is_deferred_to_unresolved() {
        let mut store = CertStore::<i64>::new();
        let id = store.append_leaf("c".to_string(), Verdict::Verified(()), Some(MarginSq(1)));
        assert_eq!(store.fresh_recheck(id), Verdict::Unresolved(()));
    }
}
