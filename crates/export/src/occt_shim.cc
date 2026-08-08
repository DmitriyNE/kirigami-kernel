// The OCCT STEP-export shim (C++ side). M6.0 is a single write-then-reload
// smoke; the M6.3 shell writer builds on the same STEPControl_Writer path.
// Strings only cross the FFI boundary — no OCCT type escapes.
#include "export/src/occt_shim.h"

#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <IFSelect_ReturnStatus.hxx>
#include <STEPControl_Reader.hxx>
#include <STEPControl_StepModelType.hxx>
#include <STEPControl_Writer.hxx>
#include <TopoDS_Shape.hxx>
#include <gp_Pnt.hxx>

#include <exception>
#include <string>

namespace kirigami {

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

    // Sew each triangle's planar face into one shell. A degenerate triangle (a
    // failed polygon/face build) is skipped, not fatal — the exact record guards
    // orientation; a collinear sliver here would only trip MakeFace.
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
    if (faces == 0) return rust::String("error: no non-degenerate triangles");

    sewing.Perform();
    TopoDS_Shape shell = sewing.SewedShape();
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

}  // namespace kirigami
