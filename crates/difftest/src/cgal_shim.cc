// The CGAL differential-oracle shim (C++ side). Phase 0 stands up the toolchain
// with a minimal smoke; the Arrangement_2 (circular kernel) oracle grows here.
#include "difftest/src/cgal_shim.h"

#include <CGAL/Gmpq.h>  // CGAL's exact rational over GMP — heavy-enough header + gmp link

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

}  // namespace kirigami
