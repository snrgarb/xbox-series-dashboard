#!/usr/bin/env bash
# One end-to-end test on a throwaway Windows VM: stages the Windows build with the test's payload, clones a VM from the
# template, runs the payload on its desktop, brings back what it wrote, and destroys the VM whatever happened.
#
#   e2e/run.sh <test> [--exe <dashboard.exe>] [--standin <standin.exe>] [--keep]
#
#   <test>       a folder under e2e/payloads/ with a run.ps1 (smoke)
#   --exe        the dashboard to test; default: this checkout's release build for Windows,
#                target/x86_64-pc-windows-gnullvm/release/dashboard.exe
#   --standin    the stand-in game and Steam; default: standin.exe from the same place
#   --keep       leave the VM running after the test, to look at it in the Proxmox console; `e2e/proxmox/vm.sh down`
#                destroys it after
#
# Never more than one test VM at a time: `vm.sh up` refuses while one exists, and so does this.
# Results land in artifacts/e2e/<test>-<time>/; exits 0 when the test passed.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/.." && pwd)"
vm="$here/proxmox/vm.sh"
usage="usage: e2e/run.sh <test> [--exe <dashboard.exe>] [--standin <standin.exe>] [--keep]"

test="${1:?$usage}"; shift
built="$repo/target/x86_64-pc-windows-gnullvm/release"
exe="$built/dashboard.exe" standin="$built/standin.exe" keep=0
while (($#)); do
    case "$1" in
        --exe) exe="${2:?--exe needs a file}"; shift 2 ;;
        --standin) standin="${2:?--standin needs a file}"; shift 2 ;;
        --keep) keep=1; shift ;;
        *) echo "unknown option $1" >&2; echo "$usage" >&2; exit 2 ;;
    esac
done
payload_src="$here/payloads/$test"
[[ -f "$payload_src/run.ps1" ]] || { echo "no test $test (no $payload_src/run.ps1)" >&2; exit 2; }
for built_exe in "$exe" "$standin"; do
    [[ -f "$built_exe" ]] && continue
    echo "no $built_exe" >&2
    echo "build it first: cargo build --release --target x86_64-pc-windows-gnullvm -j 6" >&2
    exit 2
done

log() { echo "[$(date +%H:%M:%S)] $*" >&2; }

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
cp -r "$payload_src/." "$stage/"
cp -r "$here/payloads/lib" "$stage/lib"
cp "$exe" "$stage/dashboard.exe"
cp "$standin" "$stage/standin.exe"

# The ViGEm client the tests drive their virtual pad with: managed, netstandard2.0, loaded by PowerShell 5.1 on the VM.
# Fetched once from NuGet, pinned by version and checksum, and kept under artifacts/e2e/.cache.
vigem_version="1.21.256"
vigem_sha256="4458301000b732d115521e99f9936f4edb70d6ceb3036ef158715e0e6b8902e0"
vigem="$repo/artifacts/e2e/.cache/Nefarius.ViGEm.Client-$vigem_version.dll"
if [[ ! -f "$vigem" ]] || ! echo "$vigem_sha256  $vigem" | sha256sum -c --quiet 2>/dev/null; then
    mkdir -p "$(dirname "$vigem")"
    nupkg="$(mktemp -d)"
    curl -fsSL -o "$nupkg/p.nupkg" \
        "https://api.nuget.org/v3-flatcontainer/nefarius.vigem.client/$vigem_version/nefarius.vigem.client.$vigem_version.nupkg"
    python3 -I -c 'import zipfile,sys; open(sys.argv[2],"wb").write(zipfile.ZipFile(sys.argv[1]).read("lib/netstandard2.0/Nefarius.ViGEm.Client.dll"))' \
        "$nupkg/p.nupkg" "$vigem"
    rm -rf "$nupkg"
    echo "$vigem_sha256  $vigem" | sha256sum -c --quiet || { rm -f "$vigem"; echo "ViGEm client checksum mismatch" >&2; exit 1; }
fi
cp "$vigem" "$stage/lib/Nefarius.ViGEm.Client.dll"

out="$repo/artifacts/e2e/$test-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$out"
log "testing $exe ($(sha256sum "$exe" | cut -c1-12), built $(date -r "$exe" '+%Y-%m-%d %H:%M'))"

id="$("$vm" up | tail -1)"
cleanup() {
    if ((keep)); then
        log "kept VM $id; destroy it with: e2e/proxmox/vm.sh down $id"
    else
        "$vm" down "$id" || log "could not destroy VM $id: run e2e/proxmox/vm.sh down $id"
    fi
    rm -rf "$stage"
}
trap cleanup EXIT

set +e
"$vm" desktop "$id" "$stage" "$out" 900
code=$?
set -e

[[ -f "$out/steps.txt" ]] && cat "$out/steps.txt" >&2
if ((code == 0)); then log "PASS: $test ($out)"; else log "FAIL: $test, exit $code ($out)"; fi
exit "$code"
