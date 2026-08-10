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

use crate::approx::{rat_to_f64, surd_to_f64};
use crate::brep::{Brep, EdgeGeom, FaceSurface};
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

        /// Assemble an exact B-rep (shared vertex + edge tables, faces on exact
        /// surfaces) into a `TopoDS_Shell` with edges shared **by identity** (no
        /// float-tolerance sewing), write it to `path` as a STEP file, read it back,
        /// and run `BRepCheck_Analyzer` on the reload. Returns `"ok"` on a clean
        /// write-then-reload round-trip, else `"error: <what>"`. The six flat
        /// buffers are the [`BrepBuffers`] layout. Callers should use [`write_brep`],
        /// which does the exact→`f64` cast; this raw binding takes floats directly.
        #[allow(clippy::too_many_arguments)]
        fn occt_write_brep(
            path: &str,
            verts: &[f64],
            edges: &[f64],
            beziers: &[f64],
            faces: &[f64],
            loops: &[f64],
            wires: &[f64],
            patches: &[f64],
        ) -> String;

        /// Assemble the same shell as [`occt_write_brep`] (no STEP write) and return
        /// OCCT's own topology facts as the `key=val` summary
        /// (`faces=… edges=… free=… nonmanifold=… closed=<0|1> brepcheck=<0|1>`), or
        /// `"error: <what>"`. A Π-seam shared by two faces by identity is one edge of
        /// incidence 2 — neither free nor non-manifold. The Milestone-D differential
        /// **oracle**: compared against the internal verdict, never the certificate.
        /// Callers should use [`audit_brep`], which does the cast and parses the
        /// summary into a [`ShellAudit`]; this raw binding takes floats directly.
        #[allow(clippy::too_many_arguments)]
        fn occt_brep_audit(
            verts: &[f64],
            edges: &[f64],
            beziers: &[f64],
            faces: &[f64],
            loops: &[f64],
            wires: &[f64],
            patches: &[f64],
        ) -> String;
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

/// The six flat `f64` buffers that carry an exact [`Brep`] across the FFI boundary
/// — the surface-tier analogue of [`record_to_floats`]'s single triangle buffer.
/// All indices inside the buffers are *element* indices (vertex/edge/control-point/
/// loop/half-edge), not `f64` offsets. This is the single point where the exact B-rep
/// becomes floating-point, at the last moment before OCCT; the layout is documented
/// on [`ffi::occt_write_brep`] and mirrored on the C++ side.
#[derive(Debug, Clone, PartialEq)]
pub struct BrepBuffers {
    /// 3 `f64` per vertex: `x, y, z`.
    pub verts: Vec<f64>,
    /// 5 `f64` per edge: `start_vid, end_vid, kind, bez_off, bez_deg` (`kind` 0 =
    /// `Line`, 1 = `RationalBezier`; the Bézier's control points start at
    /// control-point index `bez_off` in [`beziers`](Self::beziers)).
    pub edges: Vec<f64>,
    /// 4 `f64` per rational-Bézier control point: weighted pole `wx, wy, wz` and
    /// weight `w` (homogeneous form; the affine pole is `(wx, wy, wz) / w`).
    pub beziers: Vec<f64>,
    /// 7 `f64` per face: `surf_kind, a, b, c, d, loop_off, n_loops`. `surf_kind` 0 =
    /// `Plane` (`a..d` unused); 1 = `LinearExtrusion` (`a` = base edge id, `(b, c, d)` =
    /// ruling direction); 2 = `RationalPatch` (`a` = control-point index into
    /// [`patches`](Self::patches), `b` = u-degree, `c` = v-degree, `d` unused). The face
    /// is bounded by `n_loops` boundary loops from loop index `loop_off` in
    /// [`loops`](Self::loops): the first (`loop_off`) is the outer wire, the rest are
    /// interior holes.
    pub faces: Vec<f64>,
    /// 2 `f64` per boundary loop: `wire_off, wire_len` — the loop is `wire_len`
    /// half-edges from half-edge index `wire_off` in [`wires`](Self::wires). A face's
    /// outer wire and each of its holes is one entry here (see [`faces`](Self::faces)).
    pub loops: Vec<f64>,
    /// 2 `f64` per half-edge: `edge_id, reversed` (`reversed` 0 or 1).
    pub wires: Vec<f64>,
    /// 4 `f64` per rational-patch control point: weighted pole `wx, wy, wz` and weight
    /// `w` (homogeneous form). A `surf_kind == 2` face's `(udeg+1)·(vdeg+1)` control
    /// points are row-major (`u` outer, `v` inner) starting at its control-point index.
    pub patches: Vec<f64>,
}

/// Flatten an exact [`Brep`] into the five [`BrepBuffers`] the surface writer
/// consumes, casting each exact `Surd` vertex and each `Rat` Bézier pole / weight /
/// extrusion direction through the quarantined [`approx`](crate::approx) bridge.
/// This is the single exact→`f64` cast for the surface path (mirroring
/// [`record_to_floats`] for the mesh path).
pub fn brep_to_buffers<B: Backend>(b: &Brep<B>) -> BrepBuffers {
    let mut verts = Vec::with_capacity(b.verts().len() * 3);
    for v in b.verts() {
        for coord in v {
            verts.push(surd_to_f64(coord));
        }
    }

    let mut edges = Vec::with_capacity(b.edges().len() * 5);
    let mut beziers: Vec<f64> = Vec::new();
    for e in b.edges() {
        match &e.geom {
            EdgeGeom::Line => {
                edges.extend_from_slice(&[e.start as f64, e.end as f64, 0.0, 0.0, 0.0]);
            }
            EdgeGeom::RationalBezier(bez) => {
                let off = beziers.len() / 4; // control-point index, not f64 offset
                edges.extend_from_slice(&[
                    e.start as f64,
                    e.end as f64,
                    1.0,
                    off as f64,
                    bez.degree() as f64,
                ]);
                let wp = bez.weighted_poles();
                let w = bez.weights();
                for (pole, weight) in wp.iter().zip(w) {
                    beziers.push(rat_to_f64(&pole[0]));
                    beziers.push(rat_to_f64(&pole[1]));
                    beziers.push(rat_to_f64(&pole[2]));
                    beziers.push(rat_to_f64(weight));
                }
            }
        }
    }

    let mut faces = Vec::with_capacity(b.faces().len() * 7);
    let mut loops: Vec<f64> = Vec::new();
    let mut wires: Vec<f64> = Vec::new();
    let mut patches: Vec<f64> = Vec::new();
    for f in b.faces() {
        // The face's boundary loops, outer wire first then each hole: one `loops` entry
        // (wire_off, wire_len) per loop, its half-edges appended to `wires`. A hole-free
        // face emits exactly one loop, so the layout is byte-identical to the pre-hole
        // single-wire encoding one indirection level down.
        let loop_off = loops.len() / 2; // loop index, not f64 offset
        for lp in std::iter::once(&f.wire).chain(f.holes.iter()) {
            let wire_off = wires.len() / 2; // half-edge index, not f64 offset
            for &(eid, reversed) in lp {
                wires.push(eid as f64);
                wires.push(if reversed { 1.0 } else { 0.0 });
            }
            loops.push(wire_off as f64);
            loops.push(lp.len() as f64);
        }
        let n_loops = 1 + f.holes.len();
        match &f.surface {
            FaceSurface::Plane => {
                faces.extend_from_slice(&[
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    loop_off as f64,
                    n_loops as f64,
                ]);
            }
            FaceSurface::LinearExtrusion { base, dir } => {
                faces.extend_from_slice(&[
                    1.0,
                    *base as f64,
                    rat_to_f64(&dir[0]),
                    rat_to_f64(&dir[1]),
                    rat_to_f64(&dir[2]),
                    loop_off as f64,
                    n_loops as f64,
                ]);
            }
            FaceSurface::RationalPatch(patch) => {
                let patch_off = patches.len() / 4; // control-point index, not f64 offset
                let wp = patch.weighted_poles();
                let w = patch.weights();
                for (pole, weight) in wp.iter().zip(w) {
                    patches.push(rat_to_f64(&pole[0]));
                    patches.push(rat_to_f64(&pole[1]));
                    patches.push(rat_to_f64(&pole[2]));
                    patches.push(rat_to_f64(weight));
                }
                faces.extend_from_slice(&[
                    2.0,
                    patch_off as f64,
                    patch.udeg() as f64,
                    patch.vdeg() as f64,
                    0.0,
                    loop_off as f64,
                    n_loops as f64,
                ]);
            }
        }
    }

    BrepBuffers {
        verts,
        edges,
        beziers,
        faces,
        loops,
        wires,
        patches,
    }
}

/// Write an exact [`Brep`] to `path` as a STEP file (through the OCCT
/// `STEPControl_Writer`), the edges shared **by identity** — no float-tolerance
/// sewing — then read it back and validate the reload with `BRepCheck_Analyzer`.
/// Returns `"ok"` on a clean write-then-reload round-trip, or `"error: <what>"`.
/// The exact→`f64` cast happens here, once, via [`brep_to_buffers`]. A
/// write-then-reload check, **not** the external-kernel audit.
///
/// # Example (requires `--features step` and a writable path)
///
/// ```no_run
/// use export::brep::{Brep, EdgeGeom};
/// use lattice::{Bignum, Rat, Surd};
///
/// let r = |v: i128| Surd::<Bignum>::from_rat(Rat::from_i128(v));
/// let mut b = Brep::<Bignum>::new();
/// let v0 = b.add_vertex([r(0), r(0), r(0)]);
/// let v1 = b.add_vertex([r(1), r(0), r(0)]);
/// let v2 = b.add_vertex([r(0), r(1), r(0)]);
/// let e0 = b.add_edge(v0, v1, EdgeGeom::Line);
/// let e1 = b.add_edge(v1, v2, EdgeGeom::Line);
/// let e2 = b.add_edge(v2, v0, EdgeGeom::Line);
/// b.add_plane(vec![(e0, false), (e1, false), (e2, false)]);
/// assert_eq!(export::step::write_brep("/tmp/kirigami-tri.step", &b), "ok");
/// ```
pub fn write_brep<B: Backend>(path: &str, b: &Brep<B>) -> String {
    let bufs = brep_to_buffers(b);
    ffi::occt_write_brep(
        path,
        &bufs.verts,
        &bufs.edges,
        &bufs.beziers,
        &bufs.faces,
        &bufs.loops,
        &bufs.wires,
        &bufs.patches,
    )
}

/// Assemble an exact [`Brep`] into an OCCT shell in memory (no STEP write) and
/// return its [`ShellAudit`] facts — the Milestone-D external-kernel **differential
/// oracle** for the surface path. The result is *compared* against the internal
/// SEW-LINK / CAP-OUT verdict, never used as the certificate. A Π-seam shared by
/// two faces by identity is reported as one edge of incidence 2 — so it counts
/// toward neither `free_edges` nor `nonmanifold_edges`. The exact→`f64` cast
/// happens once, via [`brep_to_buffers`]. Returns `Err` with the shim's
/// `"error: <what>"` message on a malformed buffer or an OCCT failure.
///
/// # Example (requires `--features step`)
///
/// ```no_run
/// use export::brep::{Brep, EdgeGeom};
/// use lattice::{Bignum, Rat, Surd};
///
/// let r = |v: i128| Surd::<Bignum>::from_rat(Rat::from_i128(v));
/// let mut b = Brep::<Bignum>::new();
/// // Two triangles sharing the diagonal seam edge by identity.
/// let v0 = b.add_vertex([r(0), r(0), r(0)]);
/// let v1 = b.add_vertex([r(1), r(0), r(0)]);
/// let v2 = b.add_vertex([r(0), r(1), r(0)]);
/// let v3 = b.add_vertex([r(1), r(1), r(0)]);
/// let seam = b.add_edge(v1, v2, EdgeGeom::Line);
/// let a0 = b.add_edge(v0, v1, EdgeGeom::Line);
/// let a1 = b.add_edge(v2, v0, EdgeGeom::Line);
/// let b0 = b.add_edge(v1, v3, EdgeGeom::Line);
/// let b1 = b.add_edge(v3, v2, EdgeGeom::Line);
/// b.add_plane(vec![(a0, false), (seam, false), (a1, false)]);
/// b.add_plane(vec![(b0, false), (b1, false), (seam, true)]);
/// let audit = export::step::audit_brep(&b).expect("OCC audits the brep");
/// assert_eq!(audit.nonmanifold_edges, 0); // the seam is a manifold 2-incidence edge
/// ```
pub fn audit_brep<B: Backend>(b: &Brep<B>) -> Result<ShellAudit, String> {
    let bufs = brep_to_buffers(b);
    parse_shell_audit(&ffi::occt_brep_audit(
        &bufs.verts,
        &bufs.edges,
        &bufs.beziers,
        &bufs.faces,
        &bufs.loops,
        &bufs.wires,
        &bufs.patches,
    ))
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
    /// assert it reaches reloadable STEP files: `closure_valid` → **Verified**, the gate
    /// evaluates `VALID_solid-closure` over its stored certificate → **Verified**
    /// (`T = Rat`, stamped with the real REG-V margin), and **both** exported
    /// representations re-read clean through `BRepCheck_Analyzer`:
    ///
    /// - the **exact §10 body** ([`brep_from_closure`](crate::brep_build::brep_from_closure)
    ///   → [`write_brep`]) — the two flank `w = 0` ruled sheets sharing the fold crease by
    ///   identity, the spec §10 solid body;
    /// - the **§11 mesh** ([`shell_from_closure`](crate::shell::shell_from_closure) →
    ///   [`write_shell`]) — the triangle soup, kept as the discrete-net diagnostic.
    ///
    /// `stem` keeps the two cap branches' scratch files distinct.
    fn assert_one_joint_treatment_reloads(
        joint: &closure::Joint<lattice::Bignum>,
        t: &closure::valid::ClosureTreatment<'_, lattice::Bignum>,
        stem: &str,
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

        let is_step = |p: &str| {
            std::fs::read_to_string(p)
                .expect("step file readable")
                .starts_with("ISO-10303-21")
        };
        let scratch = |suffix: &str, write: &dyn Fn(&str) -> String| {
            let mut path = std::env::temp_dir();
            path.push(format!("{stem}-{suffix}.step"));
            let p = path.to_str().expect("utf-8 temp path");
            assert_eq!(write(p), "ok", "{suffix} body writes + reloads");
            assert!(is_step(p), "{suffix} body is not a STEP part-21 file");
            let _ = std::fs::remove_file(p);
        };

        // §10 exact body: the two flank ruled sheets sharing the crease by identity.
        let brep = crate::brep_build::brep_from_closure(joint, t, &valid);
        assert_eq!(
            brep.faces().len(),
            2,
            "the exact §10 body is the two flanks"
        );
        scratch("brep", &|p| super::write_brep(p, &brep));

        // §11 mesh: the triangle soup, kept as the discrete-net diagnostic.
        let shell = crate::shell::shell_from_closure(joint, t, &valid);
        assert!(!shell.is_empty());
        scratch("mesh", &|p| super::write_shell(p, &shell));

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

    /// D3.2a GATE — the watertight-by-identity mechanism through OCCT. A hand-built
    /// two-face B-rep (a unit square split into two triangles across the diagonal),
    /// the diagonal shared **by edge identity**, assembles with `BRep_Builder` (no
    /// sewing) into a shell whose audit shows the seam as a single 2-incidence edge:
    /// `edges=5`, `free=4`, `nonmanifold=0`, and `brepcheck` valid — so the one
    /// non-free, non-manifold edge is incident to exactly two faces. This is the
    /// external-kernel confirmation of `brep.rs`'s pure-combinatorial incidence.
    #[test]
    fn occt_brep_audits_two_faces_sharing_a_seam_by_identity() {
        use crate::brep::{Brep, EdgeGeom};
        use lattice::{Bignum, Rat, Surd};

        let r = |v: i128| Surd::<Bignum>::from_rat(Rat::from_i128(v));
        let mut b = Brep::<Bignum>::new();
        let v0 = b.add_vertex([r(0), r(0), r(0)]);
        let v1 = b.add_vertex([r(1), r(0), r(0)]);
        let v2 = b.add_vertex([r(0), r(1), r(0)]);
        let v3 = b.add_vertex([r(1), r(1), r(0)]);
        let seam = b.add_edge(v1, v2, EdgeGeom::Line);
        let a0 = b.add_edge(v0, v1, EdgeGeom::Line);
        let a1 = b.add_edge(v2, v0, EdgeGeom::Line);
        let b0 = b.add_edge(v1, v3, EdgeGeom::Line);
        let b1 = b.add_edge(v3, v2, EdgeGeom::Line);
        b.add_plane(vec![(a0, false), (seam, false), (a1, false)]);
        b.add_plane(vec![(b0, false), (b1, false), (seam, true)]);

        let audit = super::audit_brep(&b).expect("OCC audits the hand-built brep");
        assert_eq!(audit.faces, 2, "two faces: {audit:?}");
        assert_eq!(
            audit.edges, 5,
            "five distinct edges (the seam is shared): {audit:?}"
        );
        assert_eq!(
            audit.free_edges, 4,
            "the four outer edges are open: {audit:?}"
        );
        assert_eq!(
            audit.nonmanifold_edges, 0,
            "no non-manifold edge: {audit:?}"
        );
        assert!(audit.brepcheck_valid, "OCCT accepts the shell: {audit:?}");
        // The one edge that is neither free nor non-manifold is the shared seam,
        // incident to exactly two faces — watertight by identity, no sewing.
        assert_eq!(
            audit.edges - audit.free_edges - audit.nonmanifold_edges,
            1,
            "exactly one 2-incidence (shared) edge — the seam: {audit:?}"
        );
    }

    /// D3.2 GATE — the certified MITER closure through OCCT. [`brep_from_closure`]
    /// emits the two flank `w = 0` ruled sheets (`Geom_SurfaceOfLinearExtrusion` over
    /// rational-Bézier rails) sharing the fold crease middle `M` **by identity**. OCCT's
    /// audit must confirm what `brep.rs` computes combinatorially: two faces, no
    /// non-manifold edge, `brepcheck` valid, and exactly one 2-incidence edge (the shared
    /// `M`). And it must beat the mesh path: the exact shell's `free_edges` is **strictly
    /// lower** than `shell_from_closure`'s triangle soup — the crease is a shared edge, no
    /// longer two coincident-but-separate free boundaries.
    #[test]
    fn occt_audits_the_miter_brep_sharing_the_crease() {
        use crate::brep_build::brep_from_closure;
        use closure::valid::closure_valid;
        use fixtures::closure_joint::{miter_cap, one_joint, treatment_miter};

        let joint = one_joint();
        let cap = miter_cap();
        let t = treatment_miter(&cap);
        let valid = match closure_valid(&joint, &t) {
            certify_core::Verdict::Verified(v) => v,
            _ => panic!("the miter fold is CLOSURE_VALID"),
        };

        let brep = brep_from_closure(&joint, &t, &valid);
        let audit = super::audit_brep(&brep).expect("OCC audits the miter brep");
        assert_eq!(audit.faces, 2, "two flank sheets: {audit:?}");
        assert_eq!(
            audit.nonmanifold_edges, 0,
            "no non-manifold edge: {audit:?}"
        );
        assert!(
            audit.brepcheck_valid,
            "OCCT accepts each ruled face: {audit:?}"
        );
        // Exactly one edge is neither free nor non-manifold: the shared crease middle M,
        // incident to both flanks by identity — watertight-by-construction, no sewing.
        assert_eq!(
            audit.edges - audit.free_edges - audit.nonmanifold_edges,
            1,
            "exactly one 2-incidence (shared) edge — the crease middle M: {audit:?}"
        );

        // The exact-surface path shares the crease; the mesh path leaves it open. So the
        // ruled shell's free-edge count is strictly below the triangle soup's.
        let shell = crate::shell::shell_from_closure(&joint, &t, &valid);
        let mesh = super::audit_shell(&shell).expect("OCC audits the mesh shell");
        assert!(
            audit.free_edges < mesh.free_edges,
            "exact shell {} free edges < mesh {} — the crease is now shared, not open",
            audit.free_edges,
            mesh.free_edges
        );
    }

    /// D3.3 GATE — the certified LEDGE closure through OCCT. Its exact §10 body is the
    /// **same two flank sheets** as MITER: the only available LEDGE cap outline is the
    /// CAP-IN-D24 licensing square (a placeholder, not the real projected cut), whose crease
    /// edge overlaps the A+B seam `M`, so no certificate backs a flank↔cap seam. Rather than
    /// fabricate one — which OCCT would only accept as a disconnected shell (`BRepCheck`
    /// rejects a shell with an unconnected face) — the exact body emits the certified flanks
    /// and defers the exact cap to the `V_∂` real-cut slice (see `docs/vv-guide.md §8`). So
    /// OCCT must accept the two-face shell just as for MITER: two faces, `brepcheck` valid,
    /// no non-manifold edge, one 2-incidence crease edge, and the file reloads.
    #[test]
    fn occt_audits_the_ledge_exact_flank_body() {
        use crate::brep_build::brep_from_closure;
        use closure::valid::closure_valid;
        use fixtures::closure_joint::{ledge_d24, one_joint, treatment};

        let joint = one_joint();
        let d24 = ledge_d24();
        let t = treatment(&d24);
        let valid = match closure_valid(&joint, &t) {
            certify_core::Verdict::Verified(v) => v,
            _ => panic!("the ledge fold is CLOSURE_VALID"),
        };

        let brep = brep_from_closure(&joint, &t, &valid);
        let audit = super::audit_brep(&brep).expect("OCC audits the ledge brep");
        assert_eq!(
            audit.faces, 2,
            "two flank sheets, no exact cap (deferred to V_∂): {audit:?}"
        );
        assert_eq!(
            audit.nonmanifold_edges, 0,
            "no non-manifold edge: {audit:?}"
        );
        assert!(
            audit.brepcheck_valid,
            "OCCT accepts the connected two-flank shell: {audit:?}"
        );
        assert_eq!(
            audit.edges - audit.free_edges - audit.nonmanifold_edges,
            1,
            "exactly one 2-incidence edge — the shared crease middle M: {audit:?}"
        );

        // The exact shell writes a STEP file that reloads.
        let mut path = std::env::temp_dir();
        path.push("kirigami-ledge-brep.step");
        let p = path.to_str().expect("utf-8 temp path");
        assert_eq!(super::write_brep(p, &brep), "ok");
        assert!(
            std::fs::read_to_string(p)
                .expect("step file readable")
                .starts_with("ISO-10303-21"),
            "not a STEP part-21 file",
        );
        let _ = std::fs::remove_file(p);
    }

    /// Geom_BSplineCurve linkage: a planar face with one **rational-Bézier** edge (a
    /// quadratic rational arc, weights (1,2,1)) plus two straight sides writes a STEP
    /// file that reloads clean through BRepCheck. Proves the rational-curve edge
    /// builder (`Geom_BSplineCurve` + `BRepBuilderAPI_MakeEdge(curve, …)`) links and
    /// round-trips.
    #[test]
    fn occt_writes_a_brep_with_a_rational_bezier_edge() {
        use crate::bezier::RatBezier;
        use crate::brep::{Brep, EdgeGeom};
        use lattice::{Bignum, Rat, Surd};

        let q = |v: i128| Rat::<Bignum>::from_i128(v);
        let r = |v: i128| Surd::<Bignum>::from_rat(Rat::from_i128(v));
        // Rational quadratic in z=0 from (0,0,0) to (1,1,0); affine poles
        // (0,0,0),(1,0,0),(1,1,0), weights (1,2,1) — weighted poles below.
        let bez = RatBezier::new(
            vec![[q(0), q(0), q(0)], [q(2), q(0), q(0)], [q(1), q(1), q(0)]],
            vec![q(1), q(2), q(1)],
        );

        let mut b = Brep::<Bignum>::new();
        let v0 = b.add_vertex([r(0), r(0), r(0)]);
        let v1 = b.add_vertex([r(1), r(1), r(0)]);
        let v2 = b.add_vertex([r(0), r(1), r(0)]);
        let arc = b.add_edge(v0, v1, EdgeGeom::RationalBezier(bez));
        let e1 = b.add_edge(v1, v2, EdgeGeom::Line);
        let e2 = b.add_edge(v2, v0, EdgeGeom::Line);
        b.add_plane(vec![(arc, false), (e1, false), (e2, false)]);

        let mut path = std::env::temp_dir();
        path.push("kirigami-brep-bezier.step");
        let p = path.to_str().expect("utf-8 temp path");
        assert_eq!(super::write_brep(p, &b), "ok");
        assert!(
            std::fs::read_to_string(p)
                .expect("step file readable")
                .starts_with("ISO-10303-21"),
            "not a STEP part-21 file",
        );
        let _ = std::fs::remove_file(p);
    }

    /// Geom_SurfaceOfLinearExtrusion linkage: a single **ruled** face — a base line
    /// swept along +z, trimmed by its four-edge wire — writes a STEP file that
    /// reloads clean through BRepCheck. Proves the extrusion-surface face builder
    /// (`Geom_SurfaceOfLinearExtrusion` + `MakeFace` + `ShapeFix_Face` pcurve
    /// healing) links and produces a valid face.
    #[test]
    fn occt_writes_a_ruled_extrusion_face() {
        use crate::brep::{Brep, EdgeGeom, FaceSurface};
        use lattice::{Bignum, Rat, Surd};

        let r = |v: i128| Surd::<Bignum>::from_rat(Rat::from_i128(v));
        let mut b = Brep::<Bignum>::new();
        let v0 = b.add_vertex([r(0), r(0), r(0)]);
        let v1 = b.add_vertex([r(1), r(0), r(0)]);
        let v2 = b.add_vertex([r(1), r(0), r(1)]);
        let v3 = b.add_vertex([r(0), r(0), r(1)]);
        let base = b.add_edge(v0, v1, EdgeGeom::Line);
        let side1 = b.add_edge(v1, v2, EdgeGeom::Line);
        let top = b.add_edge(v2, v3, EdgeGeom::Line);
        let side2 = b.add_edge(v3, v0, EdgeGeom::Line);
        b.add_face(
            FaceSurface::LinearExtrusion {
                base,
                dir: [Rat::from_i128(0), Rat::from_i128(0), Rat::from_i128(1)],
            },
            vec![(base, false), (side1, false), (top, false), (side2, false)],
        );

        let mut path = std::env::temp_dir();
        path.push("kirigami-brep-ruled.step");
        let p = path.to_str().expect("utf-8 temp path");
        assert_eq!(super::write_brep(p, &b), "ok");
        let _ = std::fs::remove_file(p);
    }

    /// G6b interior-hole emit: a planar panel with one interior **hole** wire (a 6×6
    /// outer square, a 2×2 inner square) writes a STEP face whose inner loop OCCT
    /// accepts, and the file reloads clean through BRepCheck. Exercises the widened
    /// N-loop face record + the shim's `mf.Add(holeWire)` + `ShapeFix_Face` orientation.
    #[test]
    fn occt_writes_a_planar_holed_face() {
        use crate::brep::FaceSurface;
        use crate::brep_build::brep_holed_panel;
        use lattice::{Bignum, Rat};

        let q = |n: i128| Rat::<Bignum>::from_i128(n);
        let p = |x: i128, y: i128| [q(x), q(y), q(0)];
        let outer = [p(0, 0), p(6, 0), p(6, 6), p(0, 6)];
        let hole = [p(2, 2), p(4, 2), p(4, 4), p(2, 4)];
        let b = brep_holed_panel(FaceSurface::Plane, &outer, &[&hole]);

        let mut path = std::env::temp_dir();
        path.push("kirigami-brep-holed.step");
        let pp = path.to_str().expect("utf-8 temp path");
        assert_eq!(super::write_brep(pp, &b), "ok");
        assert!(
            std::fs::read_to_string(pp)
                .expect("step file readable")
                .starts_with("ISO-10303-21"),
            "not a STEP part-21 file",
        );
        let _ = std::fs::remove_file(pp);
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
