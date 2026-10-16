#!/usr/bin/env bash
# For every slug in both fixtures/reference and fixtures/gpui: fixtures/compare/<slug>.png
# (reference | gpui | diff) and fixtures/compare/report.txt (RMSE per slug, worst first).
# Usage: fixtures/scripts/compare.sh [slug-filter]   e.g. `just compare settings`
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
REF=$ROOT/fixtures/reference GPUI=$ROOT/fixtures/gpui OUT=$ROOT/fixtures/compare
FILTER=${1:-}
PATH="$(nix build nixpkgs#imagemagick --print-out-paths --no-link)/bin:$PATH"
mkdir -p "$OUT"
# montage draws no labels without an explicit font file
FONT=$(fc-match -f '%{file}' sans)

rows=()
for ref in "$REF"/*.png; do
  slug=$(basename "$ref" .png)
  new=$GPUI/$slug.png
  [[ -f $new && ( -z $FILTER || $slug == *$FILTER* ) ]] || continue
  # normalized RMSE (0..1) is printed in parentheses on stderr; compare exits 1 when images differ
  rmse=$(magick compare -metric RMSE "$ref" "$new" "$OUT/$slug.diff.png" 2>&1 >/dev/null || true)
  rmse=${rmse#*(} rmse=${rmse%)*}
  magick montage -font "$FONT" -pointsize 20 -geometry +4+4 -tile 3x1 -background '#333' -fill white \
    -label reference "$ref" -label gpui "$new" -label "diff $rmse" "$OUT/$slug.diff.png" "$OUT/$slug.png"
  rows+=("$rmse $slug")
  echo "$rmse $slug"
done
printf '%s\n' "${rows[@]}" | LC_ALL=C sort -rg | awk '{printf "%-8s %s\n", $1, $2}' >"$OUT/report.txt"
echo "report: $OUT/report.txt"
