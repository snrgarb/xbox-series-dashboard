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
log() { echo "[$(date +%H:%M:%S)] $*" >&2; }

# shellcheck source=e2e/stage.sh
. "$here/stage.sh"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
stage_payload "$test" "$exe" "$standin" "$stage"

out="$repo/artifacts/e2e/$test-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$out"
log "testing $exe ($(sha256sum "$exe" | cut -c1-12), built $(date -r "$exe" '+%Y-%m-%d %H:%M'))"

id="$("$vm" up | tail -1)"
# shellcheck disable=SC2317  # run by the trap
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
