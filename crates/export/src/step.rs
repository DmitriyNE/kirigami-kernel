//! The OCCT STEP-export FFI bridge (feature `step`).
//!
//! A `cxx` shim to OpenCASCADE's `STEPControl_Writer` — the real `.step` writer
//! for the certified one-joint shell. Strings and paths only cross the boundary;
//! no OCCT type escapes, and all `unsafe` is confined to the cxx glue. Floats
//! reaching the shim come through [`crate::approx`] (the quarantined exact→`f64`
//! bridge) — this crate is the shell tier, outside the `no_float` lint.
//!
//! `step` is **off by default** so the default workspace build and CI clippy
//! need no system OCCT; build with `--features step` under `nix develop` (which
//! ships `opencascade-occt`). Mirrors `difftest`'s `cgal` feature.
//!
//! # Example (requires `--features step` and a writable path)
//!
//! ```no_run
//! let status = export::step::occt_write_box_smoke("/tmp/kirigami-box.step");
//! assert_eq!(status, "ok"); // wrote a unit box and reloaded it through BRepCheck
//! ```

use crate::approx::surd_to_f64;
use crate::shell::ShellRecord;
use lattice::Backend;

#[cxx::bridge(namespace = "kirigami")]
mod ffi {
    unsafe extern "C++" {
        include!("export/src/occt_shim.h");

        /// Write a unit box to `path` as a STEP file, read it back, and run a
        /// `BRepCheck_Analyzer` on the reload. Returns `"ok"` on a clean
        /// write-then-reload round-trip, or `"error: <what>"`. The M6.0 OCCT
        /// link/write GO/NO-GO smoke; a write-then-reload check, **not** the
        /// external-kernel audit (Milestone D).
        fn occt_write_box_smoke(path: &str) -> String;

        /// Sew a triangle soup (`tris` = 9 doubles per triangle) into a
        /// `TopoDS_Shell`, write it to `path` as a STEP file, read it back, and run
        /// a `BRepCheck_Analyzer` on the reload. Returns `"ok"` on a clean
        /// write-then-reload round-trip whose reload passes BRepCheck, else
        /// `"error: <what>"`. A write-then-reload check, **not** the external-kernel
        /// audit (Milestone D). Callers should use [`write_shell`], which does the
        /// exact→`f64` cast; this raw binding takes floats directly.
        fn occt_write_shell(path: &str, tris: &[f64]) -> String;

        /// Sew the same shell as [`occt_write_shell`] (no STEP write) and return
        /// OCCT's own topology facts as a one-line `key=val` summary
        /// (`faces=… edges=… free=… nonmanifold=… closed=<0|1> brepcheck=<0|1>`),
        /// or `"error: <what>"`. The Milestone D differential **oracle**: these
        /// facts are compared against the internal SEW-LINK / CAP-OUT verdict,
        /// never trusted as the certificate. Callers should use [`audit_shell`],
        /// which does the exact→`f64` cast and parses the summary into a typed
        /// [`ShellAudit`]; this raw binding takes floats directly.
        fn occt_shell_audit(tris: &[f64]) -> String;
    }
}

pub use ffi::occt_write_box_smoke;

/// Flatten an exact [`ShellRecord`] into the writer's float buffer — 9 `f64` per
/// triangle (`v0.xyz, v1.xyz, v2.xyz`), each exact `a + b√d` vertex cast through the
/// quarantined [`surd_to_f64`] bridge. This is the single
/// point where the exact shell becomes floating-point, at the last moment before OCCT.
pub fn record_to_floats<B: Backend>(rec: &ShellRecord<B>) -> Vec<f64> {
    let mut out = Vec::with_capacity(rec.len() * 9);
    for tri in rec.tris() {
        for vertex in &tri.v {
            for coord in vertex {
                out.push(surd_to_f64(coord));
            }
        }
    }
    out
}

/// Write a certified one-joint [`ShellRecord`] to `path` as a STEP file (through the
/// OCCT `STEPControl_Writer`), then read it back and validate the reload with
/// `BRepCheck_Analyzer`. Returns `"ok"` on a clean write-then-reload round-trip, or
/// `"error: <what>"`. The exact→`f64` cast happens here, once, via
/// [`record_to_floats`]. This is a write-then-reload check, **not** the external-kernel
/// audit (Milestone D).
pub fn write_shell<B: Backend>(path: &str, rec: &ShellRecord<B>) -> String {
    ffi::occt_write_shell(path, &record_to_floats(rec))
}

/// The extended `BRepCheck`/topology facts OCCT observes about the sewn shell — the
/// external-kernel differential **oracle**'s reading, to be *compared* against the
/// internal SEW-LINK / CAP-OUT verdict, never trusted as the certificate
/// ("oracle ∧ audit, never oracle-instead-of-audit"). Produced by [`audit_shell`].
///
/// The interesting Milestone-D fact is `free_edges`: slice 1's 2:1 ruling-speed
/// overhang means the exported band is genuinely open along the crease, so OCCT
/// reports `free_edges > 0` / `closed == false` even though the internal certificate
/// is manifold — a divergence the differential harness asserts rather than hides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellAudit {
    /// Number of faces in the sewn shell.
    pub faces: usize,
    /// Number of distinct edges in the sewn shell.
    pub edges: usize,
    /// Edges incident to exactly one face — the open-boundary (non-watertight) locus.
    pub free_edges: usize,
    /// Edges incident to three or more faces — the non-manifold locus.
    pub nonmanifold_edges: usize,
    /// Whether OCCT considers the shell topologically closed (`BRep_Tool::IsClosed`).
    pub closed: bool,
    /// Whether the sewn shell passes `BRepCheck_Analyzer::IsValid()`.
    pub brepcheck_valid: bool,
}

/// Sew a certified [`ShellRecord`] into an OCCT shell in memory (no STEP write) and
/// return its extended [`ShellAudit`] facts — the Milestone D external-kernel
/// **differential oracle**. The result is *compared* against the internal SEW-LINK /
/// CAP-OUT verdict, never used as the certificate. The exact→`f64` cast happens once,
/// via [`record_to_floats`] (the same buffer [`write_shell`] emits). Returns `Err`
/// with the shim's `"error: <what>"` message on a malformed buffer or an OCCT failure.
///
/// # Example (requires `--features step`)
///
/// ```no_run
/// use fixtures::closure_joint::{ledge_d24, one_joint, treatment};
/// use closure::valid::closure_valid;
/// use certify_core::Verdict;
///
/// let joint = one_joint();
/// let d24 = ledge_d24();
/// let t = treatment(&d24);
/// let Verdict::Verified(valid) = closure_valid(&joint, &t) else {
///     panic!("the fixture is CLOSURE_VALID");
/// };
/// let shell = export::shell::shell_from_closure(&joint, &t, &valid);
/// let audit = export::step::audit_shell(&shell).expect("OCC audits the shell");
/// assert!(audit.brepcheck_valid); // OCCT accepts each face; the crease is still open
/// ```
pub fn audit_shell<B: Backend>(rec: &ShellRecord<B>) -> Result<ShellAudit, String> {
    parse_shell_audit(&ffi::occt_shell_audit(&record_to_floats(rec)))
}

/// Parse the shim's one-line `key=val` audit summary into a [`ShellAudit`]. An
/// `"error: …"` summary (malformed buffer / OCCT failure) is returned verbatim as
/// `Err`; a missing or non-numeric field is a parse `Err`.
fn parse_shell_audit(summary: &str) -> Result<ShellAudit, String> {
    if summary.starts_with("error:") {
        return Err(summary.to_string());
    }
    let mut fields = std::collections::HashMap::new();
    for tok in summary.split_whitespace() {
        let (k, v) = tok
            .split_once('=')
            .ok_or_else(|| format!("malformed audit token {tok:?} in {summary:?}"))?;
        fields.insert(k, v);
    }
    let uget = |k: &str| -> Result<usize, String> {
        fields
            .get(k)
            .ok_or_else(|| format!("audit summary missing {k:?}: {summary:?}"))?
            .parse::<usize>()
            .map_err(|e| format!("audit field {k:?} is not a usize ({e}): {summary:?}"))
    };
    Ok(ShellAudit {
        faces: uget("faces")?,
        edges: uget("edges")?,
        free_edges: uget("free")?,
        nonmanifold_edges: uget("nonmanifold")?,
        closed: uget("closed")? != 0,
        brepcheck_valid: uget("brepcheck")? != 0,
    })
}

#[cfg(test)]
mod tests {
    /// The GO/NO-GO link+run smoke: OCCT links, writes a unit-box STEP file, and
    /// reads it back through BRepCheck. Exercises the real final link of the
    /// TKDESTEP/TKBRep/… closure (an rlib build alone does not).
    #[test]
    fn occt_writes_and_reloads_a_box() {
        let mut path = std::env::temp_dir();
        path.push("kirigami-occt-smoke.step");
        let p = path.to_str().expect("utf-8 temp path");
        assert_eq!(super::occt_write_box_smoke(p), "ok");
        // The writer actually produced a non-empty file on disk.
        let bytes = std::fs::metadata(p).expect("step file exists").len();
        assert!(bytes > 0, "step file is empty");
        assert!(
            std::fs::read_to_string(p)
                .expect("step file readable")
                .starts_with("ISO-10303-21"),
            "not a STEP part-21 file",
        );
        let _ = std::fs::remove_file(p);
    }

    /// Drive one certified treatment of the physical [`one_joint`] fold end to end and
    /// assert it reaches a reloadable STEP file: `closure_valid` → **Verified**, the gate
    /// evaluates `VALID_solid-closure` over its stored certificate → **Verified**
    /// (`T = Rat`, stamped with the real REG-V margin), and the reconstructed shell writes
    /// a `.step` that re-reads clean through `BRepCheck_Analyzer`. `filename` keeps the two
    /// cap branches' scratch files distinct.
    fn assert_one_joint_treatment_reloads(
        joint: &closure::Joint<lattice::Bignum>,
        t: &closure::valid::ClosureTreatment<'_, lattice::Bignum>,
        filename: &str,
    ) -> closure::valid::CapWitness<lattice::Bignum> {
        use certify_core::Verdict;
        use closure::valid::closure_valid;
        use gate::store::CertStore;
        use lattice::{Bignum, Rat};

        // CLOSURE_VALID(0): the certified joint verdict.
        let valid = match closure_valid(joint, t) {
            Verdict::Verified(v) => v,
            other => panic!(
                "the fold is CLOSURE_VALID: {}",
                matches!(other, Verdict::Verified(_))
            ),
        };

        // Gate: VALID_solid-closure over the one stored joint, stamped with the real
        // REG-V squared margin (T = Rat — never a float enters the ledger).
        let mut store = CertStore::<Rat<Bignum>>::new();
        let j0 = store.append_leaf(
            "CLOSURE_VALID(0)".to_string(),
            Verdict::Verified(()),
            Some(t.reg_v_margin.clone()),
        );
        let complement = Verdict::Verified(());
        let (_gate_id, outcome) = store.evaluate_solid_closure(&complement, &[j0]).unwrap();
        assert!(
            matches!(outcome, Verdict::Verified(_)),
            "VALID_solid-closure must pass"
        );

        // The reconstructed shell writes a STEP file that reloads through BRepCheck.
        let shell = crate::shell::shell_from_closure(joint, t, &valid);
        assert!(!shell.is_empty());
        let mut path = std::env::temp_dir();
        path.push(filename);
        let p = path.to_str().expect("utf-8 temp path");
        assert_eq!(super::write_shell(p, &shell), "ok");
        assert!(
            std::fs::read_to_string(p)
                .expect("step file readable")
                .starts_with("ISO-10303-21"),
            "not a STEP part-21 file",
        );
        let _ = std::fs::remove_file(p);
        valid.cap
    }

    /// M-D slice-1 exit, **LEDGE** branch: the physical 90° cylinder fold with a spanning
    /// cap face runs `closure_valid` → Verified, passes the `VALID_solid-closure` gate, and
    /// its shell (two flank strips + the metric-faithful cap fan) reloads through BRepCheck.
    #[test]
    fn one_joint_ledge_writes_a_reloadable_step_shell() {
        use closure::valid::CapWitness;
        use fixtures::closure_joint::{ledge_d24, one_joint, treatment};

        let joint = one_joint();
        let d24 = ledge_d24();
        let t = treatment(&d24);
        let cap = assert_one_joint_treatment_reloads(&joint, &t, "kirigami-one-joint-ledge.step");
        assert!(
            matches!(cap, CapWitness::Ledge(_)),
            "the ledge treatment certifies via the LEDGE cap branch"
        );
    }

    /// M-D slice-1 exit, **MITER** branch: the *same* physical fold with a clean mitered
    /// corner (no separate cap face — the flanks meet directly) runs `closure_valid` →
    /// Verified, passes the `VALID_solid-closure` gate, and its shell (the two flank strips)
    /// reloads through BRepCheck.
    #[test]
    fn one_joint_miter_writes_a_reloadable_step_shell() {
        use closure::valid::CapWitness;
        use fixtures::closure_joint::{miter_cap, one_joint, treatment_miter};

        let joint = one_joint();
        let cap_outline = miter_cap();
        let t = treatment_miter(&cap_outline);
        let cap = assert_one_joint_treatment_reloads(&joint, &t, "kirigami-one-joint-miter.step");
        assert!(
            matches!(cap, CapWitness::Miter(_)),
            "the miter treatment certifies via the MITER cap branch"
        );
    }

    /// D2.1 GO/NO-GO: the extended OCCT audit links and runs on the slice-1 shell.
    /// The `occt_shell_audit` binding sews the same shell as the writer and reports
    /// topology facts; here we only assert the summary parses into a sane
    /// [`ShellAudit`] (faces/edges present, OCCT accepts each face). The oracle's
    /// *comparison* against the internal verdict is the D2.2 differential harness.
    #[test]
    fn occt_audits_the_one_joint_shell() {
        use closure::valid::closure_valid;
        use fixtures::closure_joint::{ledge_d24, one_joint, treatment};

        let joint = one_joint();
        let d24 = ledge_d24();
        let t = treatment(&d24);
        let valid = match closure_valid(&joint, &t) {
            certify_core::Verdict::Verified(v) => v,
            _ => panic!("the fixture is CLOSURE_VALID"),
        };
        let shell = crate::shell::shell_from_closure(&joint, &t, &valid);
        let audit = super::audit_shell(&shell).expect("OCC audits the sewn shell");
        assert!(audit.faces > 0, "the sewn shell has faces: {audit:?}");
        assert!(audit.edges > 0, "the sewn shell has edges: {audit:?}");
        assert!(
            audit.brepcheck_valid,
            "OCCT accepts each planar face of the sewn shell: {audit:?}"
        );
    }

    /// The `key=val` summary parser round-trips a well-formed line and rejects a
    /// malformed one — a pure test needing no OCCT link.
    #[test]
    fn shell_audit_summary_parses() {
        let a =
            super::parse_shell_audit("faces=6 edges=12 free=4 nonmanifold=0 closed=0 brepcheck=1")
                .expect("well-formed summary parses");
        assert_eq!(a.faces, 6);
        assert_eq!(a.edges, 12);
        assert_eq!(a.free_edges, 4);
        assert_eq!(a.nonmanifold_edges, 0);
        assert!(!a.closed);
        assert!(a.brepcheck_valid);

        assert!(super::parse_shell_audit("error: no non-degenerate triangles").is_err());
        assert!(super::parse_shell_audit("faces=oops edges=1").is_err());
    }
}
