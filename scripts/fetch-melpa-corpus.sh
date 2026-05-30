#!/usr/bin/env bash
# Download a random sample of .el SOURCE files from MELPA into a corpus
# directory, for use as parser/lexer test input.
#
# SECURITY: the downloaded files are ONLY read by ferrel's parser. They are
# never loaded, evaluated, byte-compiled, or otherwise executed. Tar archives
# are extracted with --no-same-owner and only their .el members are kept.
#
# Usage:
#   COUNT=50 OUTDIR=corpus scripts/fetch-melpa-corpus.sh
#
# Environment:
#   COUNT   number of MELPA packages to sample (default 50)
#   OUTDIR  directory to write .el files into (default corpus)

set -euo pipefail

COUNT="${COUNT:-50}"
OUTDIR="${OUTDIR:-corpus}"
ARCHIVE_URL="https://melpa.org/archive.json"
BASE_URL="https://melpa.org/packages"

for tool in curl jq shuf tar; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "error: required tool '$tool' not found" >&2
        exit 1
    }
done

mkdir -p "$OUTDIR"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "Fetching MELPA archive index from $ARCHIVE_URL ..."
curl -fsSL "$ARCHIVE_URL" -o "$tmp/archive.json"

# Emit one "name<TAB>version<TAB>type" row per package.
jq -r '
    to_entries[]
    | [ .key,
        (.value.ver | map(tostring) | join(".")),
        (.value.type // "single") ]
    | @tsv
' "$tmp/archive.json" >"$tmp/index.tsv"

total="$(wc -l <"$tmp/index.tsv" | tr -d ' ')"
echo "MELPA lists $total packages; sampling $COUNT."

shuf "$tmp/index.tsv" | head -n "$COUNT" >"$tmp/sample.tsv"

downloaded=0
while IFS=$'\t' read -r name ver type; do
    [ -z "$name" ] && continue
    case "$type" in
    single)
        url="$BASE_URL/${name}-${ver}.el"
        if curl -fsSL "$url" -o "$OUTDIR/${name}.el"; then
            downloaded=$((downloaded + 1))
        else
            echo "  skip (single) $name" >&2
            rm -f "$OUTDIR/${name}.el"
        fi
        ;;
    tar)
        url="$BASE_URL/${name}-${ver}.tar"
        if curl -fsSL "$url" -o "$tmp/${name}.tar"; then
            ex="$tmp/ex_${name}"
            mkdir -p "$ex"
            tar -xf "$tmp/${name}.tar" -C "$ex" --no-same-owner 2>/dev/null || true
            while IFS= read -r f; do
                cp "$f" "$OUTDIR/${name}__$(basename "$f")"
                downloaded=$((downloaded + 1))
            done < <(find "$ex" -name '*.el' 2>/dev/null)
        else
            echo "  skip (tar) $name" >&2
        fi
        ;;
    *)
        echo "  skip (unknown type '$type') $name" >&2
        ;;
    esac
done <"$tmp/sample.tsv"

echo "Downloaded $downloaded .el file(s) into $OUTDIR/"
[ "$downloaded" -gt 0 ] || {
    echo "error: no files downloaded" >&2
    exit 1
}
