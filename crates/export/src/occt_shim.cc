// The OCCT STEP-export shim (C++ side). M6.0 is a single write-then-reload
// smoke; the M6.3 shell writer builds on the same STEPControl_Writer path.
// Strings only cross the FFI boundary — no OCCT type escapes.
#include "export/src/occt_shim.h"

#include <BRep_Tool.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <IFSelect_ReturnStatus.hxx>
#include <STEPControl_Reader.hxx>
#include <STEPControl_StepModelType.hxx>
#include <STEPControl_Writer.hxx>
#include <TopAbs_ShapeEnum.hxx>
#include <TopExp.hxx>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <TopTools_ListOfShape.hxx>
#include <TopoDS_Shape.hxx>
#include <gp_Pnt.hxx>

#include <exception>
#include <string>

namespace kirigami {

// Sew a `9 * n_tris` flat buffer of `(x,y,z)` triangle corners into one shell.
// `out_faces` receives the count of non-degenerate faces added. A degenerate
// triangle (a failed polygon/face build) is skipped, not fatal — the exact
// record guards orientation; a collinear sliver here would only trip MakeFace.
// Returns a null shape iff no face survived. This is the single sewing path
// shared by the STEP writer and the differential-oracle audit, so both observe
// the identical shell.
static TopoDS_Shape sew_triangle_soup(rust::Slice<const double> tris,
                                      std::size_t& out_faces) {
  BRepBuilderAPI_Sewing sewing;
  std::size_t faces = 0;
  for (std::size_t i = 0; i + 9 <= tris.size(); i += 9) {
    gp_Pnt a(tris[i + 0], tris[i + 1], tris[i + 2]);
    gp_Pnt b(tris[i + 3], tris[i + 4], tris[i + 5]);
    gp_Pnt c(tris[i + 6], tris[i + 7], tris[i + 8]);
    BRepBuilderAPI_MakePolygon poly(a, b, c, /*Close=*/Standard_True);
    if (!poly.IsDone()) continue;
    BRepBuilderAPI_MakeFace face(poly.Wire());
    if (!face.IsDone()) continue;
    sewing.Add(face.Face());
    ++faces;
  }
  out_faces = faces;
  if (faces == 0) return TopoDS_Shape();
  sewing.Perform();
  return sewing.SewedShape();
}

rust::String occt_write_box_smoke(rust::Str path) {
  std::string p(path);
  try {
    // A unit box — the smallest solid whose STEP round-trip exercises the
    // writer, the reader, and the topology validity check.
    TopoDS_Shape box = BRepPrimAPI_MakeBox(1.0, 1.0, 1.0).Shape();

    STEPControl_Writer writer;
    if (writer.Transfer(box, STEPControl_AsIs) != IFSelect_RetDone)
      return rust::String("error: transfer failed");
    if (writer.Write(p.c_str()) != IFSelect_RetDone)
      return rust::String("error: write failed");

    STEPControl_Reader reader;
    if (reader.ReadFile(p.c_str()) != IFSelect_RetDone)
      return rust::String("error: reread failed");
    reader.TransferRoots();
    TopoDS_Shape back = reader.OneShape();
    if (back.IsNull()) return rust::String("error: reread produced a null shape");
    if (!BRepCheck_Analyzer(back).IsValid())
      return rust::String("error: reloaded shape failed BRepCheck");

    return rust::String("ok");
  } catch (const std::exception& e) {
    return rust::String(std::string("error: exception ") + e.what());
  } catch (...) {
    return rust::String("error: unknown exception");
  }
}

rust::String occt_write_shell(rust::Str path, rust::Slice<const double> tris) {
  std::string p(path);
  try {
    if (tris.size() == 0 || tris.size() % 9 != 0)
      return rust::String("error: triangle buffer is not a positive multiple of 9");

    std::size_t faces = 0;
    TopoDS_Shape shell = sew_triangle_soup(tris, faces);
    if (faces == 0) return rust::String("error: no non-degenerate triangles");
    if (shell.IsNull()) return rust::String("error: sewing produced a null shape");

    STEPControl_Writer writer;
    if (writer.Transfer(shell, STEPControl_AsIs) != IFSelect_RetDone)
      return rust::String("error: transfer failed");
    if (writer.Write(p.c_str()) != IFSelect_RetDone)
      return rust::String("error: write failed");

    STEPControl_Reader reader;
    if (reader.ReadFile(p.c_str()) != IFSelect_RetDone)
      return rust::String("error: reread failed");
    reader.TransferRoots();
    TopoDS_Shape back = reader.OneShape();
    if (back.IsNull()) return rust::String("error: reread produced a null shape");
    if (!BRepCheck_Analyzer(back).IsValid())
      return rust::String("error: reloaded shape failed BRepCheck");

    return rust::String("ok");
  } catch (const std::exception& e) {
    return rust::String(std::string("error: exception ") + e.what());
  } catch (...) {
    return rust::String("error: unknown exception");
  }
}

rust::String occt_shell_audit(rust::Slice<const double> tris) {
  try {
    if (tris.size() == 0 || tris.size() % 9 != 0)
      return rust::String("error: triangle buffer is not a positive multiple of 9");

    // Sew the SAME shell the STEP writer emits (shared helper), then read back
    // OCCT's own topology facts. This is a differential *oracle* — the facts are
    // compared against the internal SEW-LINK / CAP-OUT verdict, never trusted as
    // the certificate ("oracle ∧ audit, never oracle-instead-of-audit").
    std::size_t faces = 0;
    TopoDS_Shape shell = sew_triangle_soup(tris, faces);
    if (faces == 0) return rust::String("error: no non-degenerate triangles");
    if (shell.IsNull()) return rust::String("error: sewing produced a null shape");

    // Edge → incident-face count. Extent()==1 ⇒ a free (open-boundary) edge;
    // ==2 ⇒ a manifold interior seam; >=3 ⇒ a non-manifold edge.
    TopTools_IndexedDataMapOfShapeListOfShape edge_faces;
    TopExp::MapShapesAndAncestors(shell, TopAbs_EDGE, TopAbs_FACE, edge_faces);
    std::size_t n_edges = static_cast<std::size_t>(edge_faces.Extent());
    std::size_t free_edges = 0;
    std::size_t nonmanifold_edges = 0;
    for (Standard_Integer i = 1; i <= edge_faces.Extent(); ++i) {
      Standard_Integer deg = edge_faces.FindFromIndex(i).Extent();
      if (deg == 1)
        ++free_edges;
      else if (deg >= 3)
        ++nonmanifold_edges;
    }

    TopTools_IndexedMapOfShape face_map;
    TopExp::MapShapes(shell, TopAbs_FACE, face_map);
    std::size_t n_faces = static_cast<std::size_t>(face_map.Extent());

    bool closed = BRep_Tool::IsClosed(shell);
    bool valid = BRepCheck_Analyzer(shell).IsValid();

    std::string out = "faces=" + std::to_string(n_faces) +
                      " edges=" + std::to_string(n_edges) +
                      " free=" + std::to_string(free_edges) +
                      " nonmanifold=" + std::to_string(nonmanifold_edges) +
                      " closed=" + std::to_string(closed ? 1 : 0) +
                      " brepcheck=" + std::to_string(valid ? 1 : 0);
    return rust::String(out);
  } catch (const std::exception& e) {
    return rust::String(std::string("error: exception ") + e.what());
  } catch (...) {
    return rust::String("error: unknown exception");
  }
}

}  // namespace kirigami
