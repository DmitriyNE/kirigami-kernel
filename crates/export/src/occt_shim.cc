// The OCCT STEP-export shim (C++ side). M6.0 is a single write-then-reload
// smoke; the M6.3 shell writer builds on the same STEPControl_Writer path.
// Strings only cross the FFI boundary — no OCCT type escapes.
#include "export/src/occt_shim.h"

#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_Curve.hxx>
#include <Geom_SurfaceOfLinearExtrusion.hxx>
#include <IFSelect_ReturnStatus.hxx>
#include <STEPControl_Reader.hxx>
#include <STEPControl_StepModelType.hxx>
#include <STEPControl_Writer.hxx>
#include <ShapeFix_Face.hxx>
#include <TColStd_Array1OfInteger.hxx>
#include <TColStd_Array1OfReal.hxx>
#include <TColgp_Array1OfPnt.hxx>
#include <TopAbs_ShapeEnum.hxx>
#include <TopExp.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Shape.hxx>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Vertex.hxx>
#include <TopoDS_Wire.hxx>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <TopTools_ListOfShape.hxx>
#include <gp_Dir.hxx>
#include <gp_Pnt.hxx>

#include <exception>
#include <string>
#include <vector>

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

// Assemble the exact B-rep buffers into one `TopoDS_Shell` with edges SHARED by
// identity: every edge is built once and the same `TopoDS_Edge` (or its reverse,
// which shares the underlying TShape) is referenced by each incident face — so
// `MapShapesAndAncestors` counts a two-face seam as one edge of incidence 2, with
// no float-tolerance sewing. `out_faces` receives the count of faces added; on any
// structural fault the function writes a message to `err` and returns a null shape.
// Buffer layout is documented on `occt_write_brep` in the header.
static TopoDS_Shape build_brep_shape(rust::Slice<const double> verts,
                                     rust::Slice<const double> edges,
                                     rust::Slice<const double> beziers,
                                     rust::Slice<const double> faces,
                                     rust::Slice<const double> wires,
                                     std::size_t& out_faces, std::string& err) {
  out_faces = 0;

  // Vertices — one shared TopoDS_Vertex per table entry.
  const std::size_t nv = verts.size() / 3;
  std::vector<TopoDS_Vertex> V(nv);
  for (std::size_t i = 0; i < nv; ++i) {
    V[i] = BRepBuilderAPI_MakeVertex(
        gp_Pnt(verts[3 * i + 0], verts[3 * i + 1], verts[3 * i + 2]));
  }

  // Edges — one shared TopoDS_Edge per table entry (Line or rational Bézier).
  const std::size_t ne = edges.size() / 5;
  std::vector<TopoDS_Edge> E(ne);
  for (std::size_t e = 0; e < ne; ++e) {
    const int s = static_cast<int>(edges[5 * e + 0]);
    const int t = static_cast<int>(edges[5 * e + 1]);
    const int kind = static_cast<int>(edges[5 * e + 2]);
    if (s < 0 || static_cast<std::size_t>(s) >= nv || t < 0 ||
        static_cast<std::size_t>(t) >= nv) {
      err = "edge endpoint vertex id out of range";
      return TopoDS_Shape();
    }
    if (kind == 0) {
      BRepBuilderAPI_MakeEdge me(V[s], V[t]);
      if (!me.IsDone()) {
        err = "MakeEdge(line) failed";
        return TopoDS_Shape();
      }
      E[e] = me.Edge();
    } else {
      const int off = static_cast<int>(edges[5 * e + 3]);
      const int deg = static_cast<int>(edges[5 * e + 4]);
      const int npoles = deg + 1;
      if (deg < 1 || off < 0 ||
          static_cast<std::size_t>(4 * (off + npoles)) > beziers.size()) {
        err = "bezier control-point range out of bounds";
        return TopoDS_Shape();
      }
      TColgp_Array1OfPnt poles(1, npoles);
      TColStd_Array1OfReal weights(1, npoles);
      for (int i = 0; i < npoles; ++i) {
        const double wx = beziers[4 * (off + i) + 0];
        const double wy = beziers[4 * (off + i) + 1];
        const double wz = beziers[4 * (off + i) + 2];
        const double w = beziers[4 * (off + i) + 3];
        if (w == 0.0) {
          err = "bezier control point has zero weight";
          return TopoDS_Shape();
        }
        poles(i + 1) = gp_Pnt(wx / w, wy / w, wz / w);
        weights(i + 1) = w;
      }
      // A degree-n rational Bézier is a rational B-spline with two knots (0, 1) of
      // multiplicity n+1 — a clamped single-span curve.
      TColStd_Array1OfReal knots(1, 2);
      knots(1) = 0.0;
      knots(2) = 1.0;
      TColStd_Array1OfInteger mults(1, 2);
      mults(1) = npoles;
      mults(2) = npoles;
      Handle(Geom_BSplineCurve) curve =
          new Geom_BSplineCurve(poles, weights, knots, mults, deg);
      BRepBuilderAPI_MakeEdge me(curve, V[s], V[t]);
      if (!me.IsDone()) {
        err = "MakeEdge(bezier) failed";
        return TopoDS_Shape();
      }
      E[e] = me.Edge();
    }
  }

  // Faces — each bounded by a wire of shared edges; assembled into one shell.
  BRep_Builder builder;
  TopoDS_Shell shell;
  builder.MakeShell(shell);
  const std::size_t nf = faces.size() / 7;
  std::size_t added = 0;
  for (std::size_t f = 0; f < nf; ++f) {
    const int surf_kind = static_cast<int>(faces[7 * f + 0]);
    const int base_eid = static_cast<int>(faces[7 * f + 1]);
    const double dx = faces[7 * f + 2];
    const double dy = faces[7 * f + 3];
    const double dz = faces[7 * f + 4];
    const int woff = static_cast<int>(faces[7 * f + 5]);
    const int wlen = static_cast<int>(faces[7 * f + 6]);
    if (wlen <= 0 || woff < 0 ||
        static_cast<std::size_t>(2 * (woff + wlen)) > wires.size()) {
      err = "face wire range out of bounds";
      return TopoDS_Shape();
    }

    BRepBuilderAPI_MakeWire mw;
    for (int k = 0; k < wlen; ++k) {
      const int eid = static_cast<int>(wires[2 * (woff + k) + 0]);
      const bool rev = wires[2 * (woff + k) + 1] != 0.0;
      if (eid < 0 || static_cast<std::size_t>(eid) >= ne) {
        err = "wire references an edge id out of range";
        return TopoDS_Shape();
      }
      // Reversing shares the same underlying TShape, so identity (and therefore
      // edge incidence) is preserved across the two faces meeting on this edge.
      mw.Add(rev ? TopoDS::Edge(E[eid].Reversed()) : E[eid]);
    }
    if (!mw.IsDone()) {
      err = "MakeWire failed (wire edges do not chain)";
      return TopoDS_Shape();
    }
    const TopoDS_Wire wire = mw.Wire();

    TopoDS_Face face;
    if (surf_kind == 0) {
      // Planar face — the plane is inferred from the (coplanar-by-construction) wire.
      BRepBuilderAPI_MakeFace mf(wire, /*OnlyPlane=*/Standard_True);
      if (!mf.IsDone()) {
        err = "MakeFace(plane) failed";
        return TopoDS_Shape();
      }
      face = mf.Face();
    } else {
      // Ruled face: the base edge's curve swept along `dir`
      // (Geom_SurfaceOfLinearExtrusion), trimmed by the wire.
      if (base_eid < 0 || static_cast<std::size_t>(base_eid) >= ne) {
        err = "extrusion base edge id out of range";
        return TopoDS_Shape();
      }
      Standard_Real f0 = 0.0, l0 = 0.0;
      Handle(Geom_Curve) base = BRep_Tool::Curve(E[base_eid], f0, l0);
      if (base.IsNull()) {
        err = "extrusion base edge has no 3D curve";
        return TopoDS_Shape();
      }
      Handle(Geom_SurfaceOfLinearExtrusion) surf =
          new Geom_SurfaceOfLinearExtrusion(base, gp_Dir(dx, dy, dz));
      BRepBuilderAPI_MakeFace mf(surf, wire, /*Inside=*/Standard_True);
      if (!mf.IsDone()) {
        err = "MakeFace(extrusion) failed";
        return TopoDS_Shape();
      }
      // The wire edges carry no pcurves on the extrusion surface; heal them so the
      // face is BRepCheck-valid, without disturbing edge identity (ShapeFix adds
      // pcurve representations to the existing edges, it does not rebuild them).
      ShapeFix_Face fix(mf.Face());
      fix.Perform();
      face = fix.Face();
    }
    builder.Add(shell, face);
    ++added;
  }

  out_faces = added;
  if (added == 0) {
    err = "no faces in the brep";
    return TopoDS_Shape();
  }
  return shell;
}

rust::String occt_write_brep(rust::Str path, rust::Slice<const double> verts,
                             rust::Slice<const double> edges,
                             rust::Slice<const double> beziers,
                             rust::Slice<const double> faces,
                             rust::Slice<const double> wires) {
  std::string p(path);
  try {
    if (verts.size() % 3 != 0) return rust::String("error: verts not a multiple of 3");
    if (edges.size() % 5 != 0) return rust::String("error: edges not a multiple of 5");
    if (beziers.size() % 4 != 0) return rust::String("error: beziers not a multiple of 4");
    if (faces.size() % 7 != 0) return rust::String("error: faces not a multiple of 7");
    if (wires.size() % 2 != 0) return rust::String("error: wires not a multiple of 2");

    std::size_t nfaces = 0;
    std::string err;
    TopoDS_Shape shell =
        build_brep_shape(verts, edges, beziers, faces, wires, nfaces, err);
    if (shell.IsNull()) return rust::String(std::string("error: ") + err);

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

rust::String occt_brep_audit(rust::Slice<const double> verts,
                             rust::Slice<const double> edges,
                             rust::Slice<const double> beziers,
                             rust::Slice<const double> faces,
                             rust::Slice<const double> wires) {
  try {
    if (verts.size() % 3 != 0) return rust::String("error: verts not a multiple of 3");
    if (edges.size() % 5 != 0) return rust::String("error: edges not a multiple of 5");
    if (beziers.size() % 4 != 0) return rust::String("error: beziers not a multiple of 4");
    if (faces.size() % 7 != 0) return rust::String("error: faces not a multiple of 7");
    if (wires.size() % 2 != 0) return rust::String("error: wires not a multiple of 2");

    std::size_t nfaces = 0;
    std::string err;
    TopoDS_Shape shell =
        build_brep_shape(verts, edges, beziers, faces, wires, nfaces, err);
    if (shell.IsNull()) return rust::String(std::string("error: ") + err);

    // Edge → incident-face count, exactly as `occt_shell_audit` reads it: a
    // Π-seam shared by two faces by identity is one edge of incidence 2 (neither
    // free nor non-manifold).
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
