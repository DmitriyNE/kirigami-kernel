// The CGAL differential-oracle shim (C++ side). Phase 0 stands up the toolchain
// with a minimal smoke; Phase 5 adds the Arrangement_2 (circular kernel) oracle.
#include "difftest/src/cgal_shim.h"

#include <CGAL/Gmpq.h>  // CGAL's exact rational over GMP — heavy-enough header + gmp link

#include <CGAL/Arr_circle_segment_traits_2.h>
#include <CGAL/Arrangement_2.h>
#include <CGAL/Cartesian.h>

#include <sstream>
#include <string>

namespace kirigami {

rust::String cgal_smoke() {
  // Exact rational arithmetic: 3/4, proving CGAL headers compile and gmp links.
  CGAL::Gmpq q = CGAL::Gmpq(3) / CGAL::Gmpq(4);
  std::ostringstream os;
  os << q;  // "3/4"
  return rust::String(os.str());
}

// --- Phase-5 circular-kernel arrangement types ---------------------------------
namespace {
using Kernel = CGAL::Cartesian<CGAL::Gmpq>;  // FT = Gmpq streams as exact "n/d"
using Traits = CGAL::Arr_circle_segment_traits_2<Kernel>;
using CoordNT = Traits::CoordNT;  // a + b·√d (CGAL::_One_root_number<Gmpq>)
using Curve_2 = Traits::Curve_2;
using Arrangement = CGAL::Arrangement_2<Traits>;
using KPoint = Kernel::Point_2;
using KSegment = Kernel::Segment_2;

// A coordinate `a + b·√d` as three exact rationals "a b d".
std::string coord_triple(const CoordNT& c) {
  std::ostringstream os;
  os << c.alpha() << " " << c.beta() << " " << c.gamma();
  return os.str();
}
}  // namespace

rust::String cgal_arr_smoke() {
  // Two crossing segments (x-axis and y-axis pieces) meet at the origin.
  Curve_2 c1(KSegment(KPoint(-1, 0), KPoint(1, 0)));
  Curve_2 c2(KSegment(KPoint(0, -1), KPoint(0, 1)));
  Arrangement arr;
  CGAL::insert(arr, c1);
  CGAL::insert(arr, c2);

  // The crossing is the degree-4 vertex; endpoints are degree 1.
  std::ostringstream os;
  for (auto v = arr.vertices_begin(); v != arr.vertices_end(); ++v) {
    if (v->degree() == 4) {
      os << coord_triple(v->point().x()) << " ; " << coord_triple(v->point().y());
    }
  }
  return rust::String(os.str());
}

}  // namespace kirigami
