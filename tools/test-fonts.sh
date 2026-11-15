#!/usr/bin/env bash
# The screenshot tests' fonts, fetched into target/fonts/ (never committed: fonts are not bundled).
# Selawik is Microsoft's open-source font with Segoe UI's metrics, so text on Linux takes the room it will on Windows.
set -euo pipefail
dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/${CARGO_TARGET_DIR:-target}/fonts"
[[ "${CARGO_TARGET_DIR:-}" == /* ]] && dir="$CARGO_TARGET_DIR/fonts"
url="https://github.com/microsoft/Selawik/releases/download/1.01/Selawik_Release.zip"
sha256="3f62c51e05e3b5a1e6241cf92a371f0be2ea1183aa87b30718bbd40832a8d423"
mkdir -p "$dir"
if [[ -f "$dir/selawk.ttf" && -f "$dir/selawksb.ttf" ]]; then exit 0; fi
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
curl -fsSL -o "$tmp/selawik.zip" "$url"
echo "$sha256  $tmp/selawik.zip" | sha256sum -c --quiet
mkdir "$tmp/out"
python3 -I -c 'import zipfile,sys,os
z=zipfile.ZipFile(sys.argv[1])
for n in z.namelist():
    b=os.path.basename(n)
    if b.lower().endswith(".ttf"): open(os.path.join(sys.argv[2],b.lower()),"wb").write(z.read(n))' "$tmp/selawik.zip" "$tmp/out"
cp "$tmp"/out/*.ttf "$dir/"
ls "$dir"
