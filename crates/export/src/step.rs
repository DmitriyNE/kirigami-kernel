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
    }
}

pub use ffi::occt_write_box_smoke;

/// Flatten an exact [`ShellRecord`] into the writer's float buffer — 9 `f64` per
/// triangle (`v0.xyz, v1.xyz, v2.xyz`), each exact `a + b√d` vertex cast through the
/// quarantined [`surd_to_f64`](crate::approx::surd_to_f64) bridge. This is the single
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

    /// The Milestone-C exit: the one-joint cylinder fold runs `closure_valid` →
    /// **Verified**, the gate evaluates `VALID_solid-closure` over its stored
    /// certificate → **Verified** (`T = Rat`, stamped with the real REG-V margin), and
    /// the reconstructed shell writes to a `.step` file that reloads through BRepCheck.
    #[test]
    fn one_joint_closure_writes_a_reloadable_step_shell() {
        use certify_core::Verdict;
        use closure::valid::closure_valid;
        use fixtures::closure_joint::{ledge_d24, one_joint, treatment};
        use gate::store::CertStore;
        use lattice::{Bignum, Rat};

        let joint = one_joint();
        let d24 = ledge_d24();
        let t = treatment(&d24);

        // CLOSURE_VALID(0): the certified joint verdict.
        let valid = match closure_valid(&joint, &t) {
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
        let shell = crate::shell::shell_from_closure(&joint, &t, &valid);
        assert!(!shell.is_empty());
        let mut path = std::env::temp_dir();
        path.push("kirigami-one-joint-shell.step");
        let p = path.to_str().expect("utf-8 temp path");
        assert_eq!(super::write_shell(p, &shell), "ok");
        assert!(
            std::fs::read_to_string(p)
                .expect("step file readable")
                .starts_with("ISO-10303-21"),
            "not a STEP part-21 file",
        );
        let _ = std::fs::remove_file(p);
    }
}
