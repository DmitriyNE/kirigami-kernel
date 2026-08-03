#pragma once
#include "rust/cxx.h"

namespace kirigami {

// Build smoke — see cgal_shim.cc.
rust::String cgal_smoke();

// Circular-kernel Arrangement_2 smoke: builds the arrangement of two crossing
// segments and returns the intersection vertex as two `a b d` triples (each
// coordinate = a + b·√d), proving the circular kernel links + the Root_of_2
// extraction works. See cgal_shim.cc.
rust::String cgal_arr_smoke();

// Differential oracle: build the arrangement of the input curves and return its
// genuine multi-curve intersection vertices (degree ≥ 3). See cgal_shim.cc for
// the input format and output shape.
rust::String cgal_arrange(rust::Str input);

// Overlap-edge oracle: the arrangement's edges, each tagged with the number of
// input curves covering it (≥ 2 = an overlap / merged edge). Input curves carry
// an id. See cgal_shim.cc.
rust::String cgal_arrange_edges(rust::Str input);

// Region/boolean oracle: Boolean_set_operations_2 on circle-segment general
// polygons. Returns the connected-component count of `op` (xor|and|or) over two
// disk operands (`C cx cy r2 operand` per line). See cgal_shim.cc for the
// pinch-semantics note.
rust::String cgal_boolean_count(rust::Str input, rust::Str op);

// Total holes across all components of the boolean (annulus △ = 1).
rust::String cgal_boolean_holes(rust::Str input, rust::Str op);

// The boolean output's boundary vertices, each `xa xb xd ya yb yd` (a+b√d).
rust::String cgal_boolean_boundary(rust::Str input, rust::Str op);

}  // namespace kirigami
