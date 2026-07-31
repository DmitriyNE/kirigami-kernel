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

}  // namespace kirigami
