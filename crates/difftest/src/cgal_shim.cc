// The CGAL differential-oracle shim (C++ side). Phase 0 stands up the toolchain
// with a minimal smoke; Phase 5 adds the Arrangement_2 (circular kernel) oracle.
#include "difftest/src/cgal_shim.h"

#include <CGAL/Gmpq.h>  // CGAL's exact rational over GMP — heavy-enough header + gmp link

#include <CGAL/Arr_circle_segment_traits_2.h>
#include <CGAL/Arr_curve_data_traits_2.h>
#include <CGAL/Arrangement_2.h>
#include <CGAL/Boolean_set_operations_2.h>
#include <CGAL/Cartesian.h>
#include <CGAL/Gps_circle_segment_traits_2.h>

#include <variant>

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

// --- Phase-5/3c overlap-edge oracle (curve provenance via data traits) ----------
namespace {
// Curve data is a bitmask of originating curve ids; overlap merges by OR, so an
// edge's popcount is the number of input curves that cover it.
struct MergeOr {
  unsigned operator()(unsigned a, unsigned b) const { return a | b; }
};
using DTraits = CGAL::Arr_curve_data_traits_2<Traits, unsigned, MergeOr>;
using DCurve = DTraits::Curve_2;
using DArrangement = CGAL::Arrangement_2<DTraits>;
}  // namespace

// Build the arrangement and return one line PER EDGE: `n xa xb xd ya yb yd  ua ub
// ud va vb vd` where `n` = the number of input curves covering the edge (an
// overlap edge has n ≥ 2 — our merged coincident edge; a single curve has n = 1 —
// a residual/plain edge), and the two coordinate triples are the edge's endpoints
// (a + b·√d). Input curves carry an id: `S x1 y1 x2 y2 id` / `C cx cy r2 id`.
rust::String cgal_arrange_edges(rust::Str input) {
  std::istringstream lines{std::string(input)};
  std::string line;
  std::vector<DCurve> curves;
  while (std::getline(lines, line)) {
    std::istringstream ts(line);
    std::string kind;
    if (!(ts >> kind)) continue;
    if (kind == "S") {
      std::string x1, y1, x2, y2;
      unsigned id;
      ts >> x1 >> y1 >> x2 >> y2 >> id;
      Curve_2 base(KSegment(KPoint(parse_q(x1), parse_q(y1)),
                            KPoint(parse_q(x2), parse_q(y2))));
      curves.emplace_back(base, 1u << id);
    } else if (kind == "C") {
      std::string cx, cy, r2;
      unsigned id;
      ts >> cx >> cy >> r2 >> id;
      Kernel::Circle_2 circ(KPoint(parse_q(cx), parse_q(cy)), parse_q(r2));
      Curve_2 base(circ);
      curves.emplace_back(base, 1u << id);
    }
  }
  DArrangement arr;
  CGAL::insert(arr, curves.begin(), curves.end());

  std::ostringstream os;
  for (auto e = arr.edges_begin(); e != arr.edges_end(); ++e) {
    unsigned mask = e->curve().data();
    os << __builtin_popcount(mask) << " " << coord_triple(e->source()->point().x())
       << " " << coord_triple(e->source()->point().y()) << " "
       << coord_triple(e->target()->point().x()) << " "
       << coord_triple(e->target()->point().y()) << "\n";
  }
  return rust::String(os.str());
}

// --- 3d region/boolean oracle A: Boolean_set_operations_2 on circle-segment -------
namespace {
using GpsTraits = CGAL::Gps_circle_segment_traits_2<Kernel>;
using GPolygon = GpsTraits::Polygon_2;
using GPolygonWH = GpsTraits::Polygon_with_holes_2;
using GCurve = GpsTraits::Curve_2;
using GXcv = GpsTraits::X_monotone_curve_2;
using GPoint = GpsTraits::Point_2;

// A full circle as a CCW general polygon (its two x-monotone semicircle arcs).
GPolygon circle_polygon(const CGAL::Gmpq& cx, const CGAL::Gmpq& cy, const CGAL::Gmpq& r2) {
  KPoint center(cx, cy);
  Kernel::Circle_2 circ(center, r2);  // default orientation is counterclockwise
  GpsTraits traits;
  GCurve curve(circ);
  std::vector<std::variant<GPoint, GXcv>> objs;
  traits.make_x_monotone_2_object()(curve, std::back_inserter(objs));
  GPolygon pgn;
  for (const auto& o : objs) {
    if (const GXcv* arc = std::get_if<GXcv>(&o)) pgn.push_back(*arc);
  }
  return pgn;
}
}  // namespace

// The number of connected components (polygons-with-holes) of a boolean `op` over
// two operands built from the input disks. Input: `C cx cy r2 operand` per line
// (operand 0 = A, 1 = B; rationals num/den). `op` = "xor" | "and" | "or".
//
// NOTE on semantics: CGAL joins regions that meet only at a **pinch point** into one
// polygon-with-holes (closed-set connectivity), whereas our π₀ separates them
// (open-cell edge-adjacency — spec §6: "π₀ keeps them separate faces, CAP-OUT-LINK
// rejects the vertex"). So counts agree only on the NON-pinching cases (e.g. ∩ of
// two overlapping disks = one lens); △ of overlapping disks is pinched (CGAL 1, our
// π₀ 2). The harness compares on the non-pinching case and documents the rest.
rust::String cgal_boolean_count(rust::Str input, rust::Str op) {
  CGAL::General_polygon_set_2<GpsTraits> a, b;
  std::istringstream lines{std::string(input)};
  std::string line;
  while (std::getline(lines, line)) {
    std::istringstream ts(line);
    std::string kind, cx, cy, r2;
    unsigned operand;
    if (!(ts >> kind)) continue;
    if (kind != "C") continue;
    ts >> cx >> cy >> r2 >> operand;
    GPolygon p = circle_polygon(parse_q(cx), parse_q(cy), parse_q(r2));
    if (operand == 0) {
      a.join(p);
    } else {
      b.join(p);
    }
  }
  CGAL::General_polygon_set_2<GpsTraits> r = a;
  std::string o(op);
  if (o == "xor") {
    r.symmetric_difference(b);
  } else if (o == "and") {
    r.intersection(b);
  } else {
    r.join(b);
  }
  std::vector<GPolygonWH> res;
  r.polygons_with_holes(std::back_inserter(res));
  std::ostringstream os;
  os << res.size();
  return rust::String(os.str());
}

}  // namespace kirigami
