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
    }
}

pub use ffi::occt_write_box_smoke;

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
}
