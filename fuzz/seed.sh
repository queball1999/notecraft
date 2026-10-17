#!/usr/bin/env bash
# Fill fuzz/corpus/<target>/ from the testdata folders (PR 1.14). Run from anywhere.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
data="$here/../testdata"

seed() { # target, then source files
  local target="$1"
  shift
  mkdir -p "$here/corpus/$target"
  for f in "$@"; do
    [ -f "$f" ] || continue
    name="${f#"$data"/}"
    cp "$f" "$here/corpus/$target/${name//[\/ ]/_}"
  done
}

mapfile -t pages < <(find "$data/markdown" "$data/notecraft" "$data/notebooks" -type f \( -name '*.md' -o -name '*.notecraft' \))
mapfile -t tomls < <(find "$data/notebooks" -type f -name '*.toml')

for t in markdown frontmatter containers canvas_block; do
  seed "$t" "${pages[@]}"
done
seed notebook_toml "${tomls[@]}"
seed section_toml "${tomls[@]}"
mkdir -p "$here/corpus/recovery_journal"
printf '%s' '{"version":1,"path":"/n/A.md","text":"hello\n"}' >"$here/corpus/recovery_journal/entry.json"
echo "seeded $here/corpus"
