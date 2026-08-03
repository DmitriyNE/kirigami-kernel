#pragma once
#include "rust/cxx.h"

namespace kirigami {

// Phase-0 build smoke — see cgal_shim.cc.
rust::String cgal_smoke();

// Phase-5 circular-kernel Arrangement_2 smoke: builds the arrangement of two
// crossing segments and returns the intersection vertex as two `a b d` triples
// (each coordinate = a + b·√d), proving the circular kernel links + the
// Root_of_2 extraction works. See cgal_shim.cc.
rust::String cgal_arr_smoke();

// Phase-5 differential oracle: build the arrangement of the input curves and
// return its genuine multi-curve intersection vertices (degree ≥ 3). See
// cgal_shim.cc for the input format and output shape.
rust::String cgal_arrange(rust::Str input);

// Phase-5/3c overlap-edge oracle: the arrangement's edges, each tagged with the
// number of input curves covering it (≥ 2 = an overlap / merged edge). Input
// curves carry an id. See cgal_shim.cc.
rust::String cgal_arrange_edges(rust::Str input);

// Slice-3d region/boolean oracle A: Boolean_set_operations_2 on circle-segment
// general polygons. Returns the connected-component count of `op` (xor|and|or) over
// two disk operands (`C cx cy r2 operand` per line). See cgal_shim.cc for the
// pinch-semantics note.
rust::String cgal_boolean_count(rust::Str input, rust::Str op);

// Slice-3e Option-B: total holes across all components of the boolean (annulus △ = 1).
rust::String cgal_boolean_holes(rust::Str input, rust::Str op);

}  // namespace kirigami
