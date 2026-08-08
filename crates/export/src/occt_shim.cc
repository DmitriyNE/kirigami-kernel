// The OCCT STEP-export shim (C++ side). M6.0 is a single write-then-reload
// smoke; the M6.3 shell writer builds on the same STEPControl_Writer path.
// Strings only cross the FFI boundary — no OCCT type escapes.
#include "export/src/occt_shim.h"

#include <BRepCheck_Analyzer.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <IFSelect_ReturnStatus.hxx>
#include <STEPControl_Reader.hxx>
#include <STEPControl_StepModelType.hxx>
#include <STEPControl_Writer.hxx>
#include <TopoDS_Shape.hxx>

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

}  // namespace kirigami
