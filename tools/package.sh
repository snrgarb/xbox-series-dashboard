#!/usr/bin/env bash
# Packages a Windows build of the Dashboard (docs/release.md): the portable zip and the per-user installer, from
# dashboard.exe, WebView2Loader.dll, the README and the third-party licences, and nothing else.
#
#   tools/package.sh <dashboard.exe> <out dir>
#
# WebView2Loader.dll is taken from beside <dashboard.exe>, where the Windows build puts it (CI copies it there from
# webview2-com-sys's build): Microsoft's redistributable WebView2 loader, which Discord messages loads at run time
# (#197, docs/discord-web.md). It is never committed.
#
# Writes into <out dir>:
#   XboxSeriesDashboard-<version>-portable.zip   a folder XboxSeriesDashboard-<version>\ with the four files
#   XboxSeriesDashboard-<version>-setup.exe      the installer (packaging/installer.nsi)
#   SHA256SUMS.txt
# and the folder they were made from, stage\XboxSeriesDashboard-<version>\.
#
# Needs makensis (NSIS 3) and cargo-about on the path, or named by $MAKENSIS and $CARGO_ABOUT; cargo for
# cargo-about's look at the dependencies. The version is the workspace's (Cargo.toml, [workspace.package]).
#
# Signing: with $SIGN_COMMAND set, it is run as `$SIGN_COMMAND <file>` on dashboard.exe before anything is packed,
# and by makensis on the installer and on the uninstaller inside it. Without it, everything is unsigned.
# $SOURCE_DATE_EPOCH, else the last commit's time, is every file's time in the zip.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
usage="usage: tools/package.sh <dashboard.exe> <out dir>"
exe="${1:?$usage}"
out="${2:?$usage}"
makensis="${MAKENSIS:-makensis}"
cargo_about="${CARGO_ABOUT:-cargo-about}"

[[ -f "$exe" ]] || { echo "no $exe" >&2; exit 2; }
loader="$(dirname "$exe")/WebView2Loader.dll"
[[ -f "$loader" ]] || { echo "no WebView2Loader.dll beside $exe" >&2; exit 2; }
version="$(awk '/^\[workspace\.package\]/ { section = 1; next } /^\[/ { section = 0 }
    section && $1 == "version" { gsub(/"/, "", $3); print $3 }' "$repo/Cargo.toml")"
# The installer's version resource takes four numbers: a version with anything after x.y.z can't be packaged as is.
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "the workspace's version is not x.y.z: '$version'" >&2; exit 2; }

name="XboxSeriesDashboard-$version"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
stage="$out/stage/$name"
rm -rf "$out/stage" "$out/$name-portable.zip" "$out/$name-setup.exe" "$out/SHA256SUMS.txt"
mkdir -p "$stage"

cp "$exe" "$stage/dashboard.exe"
cp "$loader" "$stage/WebView2Loader.dll"
# Windows' line ends, for Notepad.
sed "s/@VERSION@/$version/g; s/\$/\r/" "$repo/packaging/README.txt" >"$stage/README.txt"
# The licences of every crate dashboard.exe is built from, its Windows dependencies included; a licence
# packaging/about.toml doesn't accept fails here.
"$cargo_about" generate --fail --locked --manifest-path "$repo/crates/app/Cargo.toml" \
    --config "$repo/packaging/about.toml" --output-file "$stage/THIRD-PARTY-LICENSES.txt" "$repo/packaging/about.hbs"
sed -i 's/$/\r/' "$stage/THIRD-PARTY-LICENSES.txt"

# What ships is exactly these four: no font, no sound or video, no picture of anyone's mark.
shipped="$(cd "$stage" && find . -type f | sed 's|^\./||' | LC_ALL=C sort | tr '\n' ' ')"
if [[ "$shipped" != "README.txt THIRD-PARTY-LICENSES.txt WebView2Loader.dll dashboard.exe " ]]; then
    echo "the package would hold more or less than dashboard.exe, WebView2Loader.dll, README.txt and THIRD-PARTY-LICENSES.txt: $shipped" >&2
    exit 1
fi

if [[ -n "${SIGN_COMMAND:-}" ]]; then
    echo "signing dashboard.exe"
    $SIGN_COMMAND "$stage/dashboard.exe"
fi

# The zip: one folder, every file's time the same, entries in a fixed order, no extra attributes.
epoch="${SOURCE_DATE_EPOCH:-$(git -C "$repo" log -1 --format=%ct 2>/dev/null || date +%s)}"
find "$stage" -exec touch -h -d "@$epoch" {} +
(cd "$out/stage" && find "$name" -type f | LC_ALL=C sort | TZ=UTC zip -q -X -D "$out/$name-portable.zip" -@)

nsis_args=(-V2 -INPUTCHARSET UTF8 "-DVERSION=$version" "-DSTAGE=$stage" "-DOUTFILE=$out/$name-setup.exe")
[[ -n "${SIGN_COMMAND:-}" ]] && nsis_args+=("-DSIGN_COMMAND=$SIGN_COMMAND")
"$makensis" "${nsis_args[@]}" "$repo/packaging/installer.nsi"

(cd "$out" && sha256sum "$name-portable.zip" "$name-setup.exe" >SHA256SUMS.txt)
echo "packaged $version$([[ -n "${SIGN_COMMAND:-}" ]] && echo ', signed' || echo ', unsigned') in $out:"
sed 's/^/  /' "$out/SHA256SUMS.txt"
