# Pending spec edits (queued for the next cut; none warrant a recut alone)

1. **Implementation profile — canonical D24 decomposition (from the round-22 opener):**
   Every input circle or arc is decomposed exactly into simple, x-monotone arc pieces before DCEL insertion.
   - a whole circle splits at its exact x-extremal points (degree-2 lattice coordinates);
   - with the tag chart axis-aligned, the half-angle pole *is* the x-min extremal point — extremal splitting subsumes pole splitting; the alignment convention is normative;
   - no emitted half-edge spans more than one simple point-set arc;
   - winding remains provenance on the original curve, never multiplicity on a DCEL edge.
2. **CAP-OUT wording**: "embeddedness structural for D24" → "structural for D24 *after canonical decomposition* (a full circle is not an embedded edge)".
