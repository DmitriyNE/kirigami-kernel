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

}  // namespace kirigami
