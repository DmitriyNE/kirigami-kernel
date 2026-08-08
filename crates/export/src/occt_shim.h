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

}  // namespace kirigami
