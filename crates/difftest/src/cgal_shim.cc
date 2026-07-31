// The CGAL differential-oracle shim (C++ side). Phase 0 stands up the toolchain
// with a minimal smoke; Phase 5 adds the Arrangement_2 (circular kernel) oracle.
#include "difftest/src/cgal_shim.h"

#include <CGAL/Gmpq.h>  // CGAL's exact rational over GMP — heavy-enough header + gmp link

#include <CGAL/Arr_circle_segment_traits_2.h>
#include <CGAL/Arrangement_2.h>
#include <CGAL/Cartesian.h>

#include <sstream>
#include <string>
#include <vector>

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

namespace {
CGAL::Gmpq parse_q(const std::string& s) {
  std::istringstream is(s);
  CGAL::Gmpq q;
  is >> q;  // parses "num/den" or an integer
  return q;
}
}  // namespace

// Build the arrangement of the input curves (one per line: `S x1 y1 x2 y2` for a
// segment, `C cx cy r2` for a full circle; rationals as "num/den") and return the
// genuine multi-curve intersection vertices — those of degree ≥ 3. Segment
// endpoints (degree 1) and a circle's own x-monotone-split extrema (degree 2) are
// excluded, so no curve provenance is needed. Each output line is the vertex as
// `xa xb xd ya yb yd` (coordinate = a + b·√d, six exact rationals).
rust::String cgal_arrange(rust::Str input) {
  std::istringstream lines{std::string(input)};
  std::string line;
  std::vector<Curve_2> curves;
  while (std::getline(lines, line)) {
    std::istringstream ts(line);
    std::string kind;
    if (!(ts >> kind)) continue;
    if (kind == "S") {
      std::string x1, y1, x2, y2;
      ts >> x1 >> y1 >> x2 >> y2;
      curves.emplace_back(KSegment(KPoint(parse_q(x1), parse_q(y1)),
                                   KPoint(parse_q(x2), parse_q(y2))));
    } else if (kind == "C") {
      std::string cx, cy, r2;
      ts >> cx >> cy >> r2;
      Kernel::Circle_2 circ(KPoint(parse_q(cx), parse_q(cy)), parse_q(r2));
      curves.emplace_back(circ);
    }
  }
  Arrangement arr;
  CGAL::insert(arr, curves.begin(), curves.end());

  std::ostringstream os;
  for (auto v = arr.vertices_begin(); v != arr.vertices_end(); ++v) {
    if (v->degree() >= 3) {
      os << coord_triple(v->point().x()) << " " << coord_triple(v->point().y()) << "\n";
    }
  }
  return rust::String(os.str());
}

}  // namespace kirigami
