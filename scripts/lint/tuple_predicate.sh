#!/usr/bin/env bash
# Tuple-predicate rule (spec §8.2 / glossary): predicates on multi-component
# objects name the tuple (the displayed minor/cross form IS the predicate); the
# adjective "proportional" is ambiguous and banned in new normative text and
# doc-comments.
#
# STARTER SCOPE: Rust doc-comments (///, //!) in crates/. Extending this to
# spec/ needs an allow-list for the spec's own meta-discussion of the rule
# (the frozen spec explains *why* "proportional" is banned, using the word),
# so spec-text scanning lands when spec text is next edited, not before.
set -eu
root="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
hits="$(grep -rnE '^[[:space:]]*//(/|!).*proportional' "$root/crates" 2>/dev/null || true)"
if [ -n "$hits" ]; then
  printf 'tuple-predicate: FAIL — banned adjective "proportional" in a doc-comment (name the tuple):\n%s\n' "$hits"
  exit 1
fi
echo "tuple-predicate: OK (code doc-comments; spec-text scan is a TODO)."
