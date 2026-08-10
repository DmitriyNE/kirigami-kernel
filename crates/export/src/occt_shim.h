#pragma once
#include "rust/cxx.h"

namespace kirigami {

// M6.0 OCCT link/write smoke: build a unit box, transfer it to a
// STEPControl_Writer, write `path` as a STEP file, read it back with a
// STEPControl_Reader, and run a BRepCheck_Analyzer validity pass on the
// reloaded shape. Returns "ok" on a clean write-then-reload round-trip, or
// "error: <what>" otherwise. Proves the OCCT STEP writer/reader links and
// round-trips under `nix develop` — the GO/NO-GO for the M6 STEP export. This
// is a write-then-reload check (does the file we wrote parse back into a valid
// OCCT shape), NOT the external-kernel audit (Milestone D).
rust::String occt_write_box_smoke(rust::Str path);

// M6.3 shell writer: build a sewn shell from a triangle soup and write it to `path`
// as a STEP file, then read it back and run a BRepCheck_Analyzer validity pass on the
// reload. `tris` is a flat run of 9 doubles per triangle (v0.xyz, v1.xyz, v2.xyz);
// its length must be a positive multiple of 9. Each triangle becomes a planar face
// (a closed 3-point polygon wire); the faces are sewn (BRepBuilderAPI_Sewing) into a
// TopoDS_Shell, transferred through STEPControl_Writer, and re-read. Degenerate
// triangles (a failed polygon/face build) are skipped, not fatal. Returns "ok" on a
// clean write-then-reload round-trip whose reload passes BRepCheck, else
// "error: <what>". A write-then-reload check (does the file we wrote parse back into a
// valid OCCT shape), NOT the external-kernel audit (Milestone D).
rust::String occt_write_shell(rust::Str path, rust::Slice<const double> tris);

// Milestone D differential oracle: sew the SAME shell `occt_write_shell` emits
// (shared helper, no STEP write) and report OCCT's own topology facts as a
// one-line `key=val` summary — `faces=<n> edges=<n> free=<n> nonmanifold=<n>
// closed=<0|1> brepcheck=<0|1>`. `free` counts edges incident to exactly one
// face (open boundary); `nonmanifold` counts edges incident to >=3 faces;
// `closed` is BRep_Tool::IsClosed; `brepcheck` is BRepCheck_Analyzer::IsValid.
// `tris` is a flat run of 9 doubles per triangle (length a positive multiple of
// 9). Returns "error: <what>" on a malformed buffer or an OCCT failure. These
// facts are COMPARED against the internal SEW-LINK / CAP-OUT verdict, never
// trusted as the certificate ("oracle ∧ audit, never oracle-instead-of-audit").
rust::String occt_shell_audit(rust::Slice<const double> tris);

// M-D slice-3 surface writer: assemble an exact B-rep — a shared vertex table, a
// shared edge table (each edge built ONCE and referenced by both incident faces,
// so watertightness is by *identity*, not float-tolerance sewing), and faces on
// exact surfaces — into a `TopoDS_Shell`, write it to `path` as a STEP file, then
// read it back and run a `BRepCheck_Analyzer` validity pass on the reload. Returns
// "ok" on a clean write-then-reload round-trip whose reload passes BRepCheck, else
// "error: <what>". A write-then-reload check, NOT the external-kernel audit.
//
// Seven flat `double` buffers carry the IR (all indices are element indices, not
// double offsets):
//   verts   — 3 per vertex: x, y, z.
//   edges   — 5 per edge: start_vid, end_vid, kind, bez_off, bez_deg.
//             kind 0 = straight Line (bez_* ignored); kind 1 = rational Bézier,
//             whose `bez_deg + 1` control points start at control-point index
//             `bez_off` in `beziers`.
//   beziers — 4 per rational-Bézier control point: weighted pole wx, wy, wz, and
//             weight w (the homogeneous form; affine pole is (wx,wy,wz)/w).
//   faces   — 7 per face: surf_kind, a, b, c, d, loop_off, n_loops.
//             surf_kind 0 = Plane (a..d ignored); 1 =
//             Geom_SurfaceOfLinearExtrusion of edge a's curve along (b,c,d);
//             2 = rational Geom_BSplineSurface (patch) whose (b+1)*(c+1) control
//             points start at control-point index a in `patches`, with u-degree b
//             and v-degree c (d ignored). The face is bounded by `n_loops`
//             boundary loops starting at loop index `loop_off` in `loops`: the
//             first is the outer wire, the rest are interior holes.
//   loops   — 2 per boundary loop: wire_off, wire_len. The loop is `wire_len`
//             half-edges starting at half-edge index `wire_off` in `wires`.
//   wires   — 2 per half-edge: edge_id, reversed (0 or 1).
//   patches — 4 per rational-patch control point: weighted pole wx, wy, wz, and
//             weight w (homogeneous). A patch face's control points are row-major
//             (u outer, v inner) starting at its control-point index.
rust::String occt_write_brep(rust::Str path, rust::Slice<const double> verts,
                             rust::Slice<const double> edges,
                             rust::Slice<const double> beziers,
                             rust::Slice<const double> faces,
                             rust::Slice<const double> loops,
                             rust::Slice<const double> wires,
                             rust::Slice<const double> patches);

// M-D slice-3 surface audit (differential ORACLE): assemble the SAME shell
// `occt_write_brep` emits (shared builder, no STEP write) and report OCCT's own
// topology facts as the one-line `key=val` summary `occt_shell_audit` uses
// (`faces=… edges=… free=… nonmanifold=… closed=<0|1> brepcheck=<0|1>`), or
// "error: <what>". `free` counts edges incident to exactly one face, `nonmanifold`
// edges incident to >=3 faces; a Π-seam shared by two faces *by identity* shows up
// as neither (incidence 2). These facts are COMPARED against the internal
// SEW-LINK / CAP-OUT verdict, never trusted as the certificate. Buffer layout is
// identical to `occt_write_brep`.
rust::String occt_brep_audit(rust::Slice<const double> verts,
                             rust::Slice<const double> edges,
                             rust::Slice<const double> beziers,
                             rust::Slice<const double> faces,
                             rust::Slice<const double> loops,
                             rust::Slice<const double> wires,
                             rust::Slice<const double> patches);

}  // namespace kirigami
