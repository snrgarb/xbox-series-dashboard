#!/usr/bin/env bash
# One end-to-end test on the TV PC, in safe mode: stages the payload as e2e/run.sh does, copies it to one folder in the
# TV user's %TEMP%, runs it on the TV's desktop through a one-off interactive scheduled task, brings back what it
# wrote, and removes the task, the folder and whatever the test left running, whatever happened.
#
#   e2e/htpc/run.sh <test> [--exe <dashboard.exe>] [--standin <standin.exe>] [--timeout <minutes>] [--keep]
#
#   <test>       a folder under e2e/payloads/ with a run.ps1 (smoke)
#   --exe        the dashboard to test; default: this checkout's release build for Windows,
#                target/x86_64-pc-windows-gnullvm/release/dashboard.exe
#   --standin    the stand-in games and stores; default: standin.exe from the same place
#   --timeout    how long the test may take before it is given up on and ended (15)
#   --keep       leave the test's folder on the TV PC, to look at; the task and the processes still go
#
#   HTPC_HOST    the TV PC, as ssh knows it (htpc-win)
#
# Safe mode (e2e/payloads/lib/E2E.psm1) writes nothing to the registry, reads none of the TV's own games, never turns
# the machine off or restarts it (--dry-power) and installs nothing; everything it makes is in the test's folder. The
# test does not start while the TV is in use: a full-screen window other than the desktop in front, or the Dashboard
# or Playnite running (exit 3).
#
# Results land in artifacts/e2e/htpc-<test>-<time>/; exits with the test's code: 0 passed, 1 failed, 3 the TV was in use,
# 124 timed out, 2 for a bad call.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo="$(cd "$here/.." && pwd)"
usage="usage: e2e/htpc/run.sh <test> [--exe <dashboard.exe>] [--standin <standin.exe>] [--timeout <minutes>] [--keep]"

test="${1:?$usage}"; shift
built="$repo/target/x86_64-pc-windows-gnullvm/release"
exe="$built/dashboard.exe" standin="$built/standin.exe" keep=0 minutes=15
while (($#)); do
    case "$1" in
        --exe) exe="${2:?--exe needs a file}"; shift 2 ;;
        --standin) standin="${2:?--standin needs a file}"; shift 2 ;;
        --timeout) minutes="${2:?--timeout needs minutes}"; shift 2 ;;
        --keep) keep=1; shift ;;
        *) echo "unknown option $1" >&2; echo "$usage" >&2; exit 2 ;;
    esac
done
# The name goes into cmd command lines on the TV PC: nothing but a plain folder name.
[[ "$test" =~ ^[A-Za-z0-9_-]+$ ]] || { echo "bad test name: $test" >&2; exit 2; }
if [[ ! "$minutes" =~ ^[1-9][0-9]*$ ]]; then echo "bad --timeout: $minutes" >&2; exit 2; fi

host="${HTPC_HOST:-htpc-win}"
task="XsdE2E"
# The folder on the TV PC, as its cmd says it and as scp, starting in the user's profile, reaches it.
remote="%TEMP%\\xsd-e2e\\$test"
remote_scp="AppData/Local/Temp/xsd-e2e/$test"

log() { echo "[$(date +%H:%M:%S)] $*" >&2; }
# A command for the TV PC's cmd. ssh's own warnings (post-quantum key exchange) go to stderr, which is dropped.
on_tv() { ssh -o BatchMode=yes -o ConnectTimeout=20 "$host" "$1" 2>/dev/null; }
copy() { scp -q -r -o BatchMode=yes -o ConnectTimeout=20 "$@" 2>/dev/null; }
# Whether what the TV PC's cmd printed for $1 has $2 in it.
tv_says() { local said; said="$(on_tv "$1" || true)"; [[ "$said" == *"$2"* ]]; }

# shellcheck source=e2e/stage.sh
. "$here/stage.sh"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
stage_payload "$test" "$exe" "$standin" "$stage"
cp "$here/htpc/launcher.ps1" "$here/htpc/cleanup.ps1" "$stage/"
# cmd reads a batch file best with Windows' line ends.
printf '@echo off\r\npowershell -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "%%~dp0launcher.ps1"\r\n' >"$stage/run.cmd"

out="$repo/artifacts/e2e/htpc-$test-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$out"
log "testing $exe ($(sha256sum "$exe" | cut -c1-12), built $(date -r "$exe" '+%Y-%m-%d %H:%M')) on $host"

# One test at a time: a task of ours still there is another run, or one killed before it could clean up.
if ! tv_says "echo ready" ready; then
    echo "cannot reach $host over ssh" >&2
    exit 2
fi
if tv_says "schtasks /query /tn $task >nul 2>&1 && echo there" there; then
    echo "a task $task is already on $host: another test is running, or one was killed before it cleaned up." >&2
    echo "if none is running: ssh $host schtasks /delete /tn $task /f" >&2
    exit 3
fi

# shellcheck disable=SC2317  # run by the trap
cleanup() {
    local code=$?
    log "cleaning up on $host"
    on_tv "schtasks /end /tn $task >nul 2>&1" || true
    on_tv "if exist \"$remote\\cleanup.ps1\" powershell -NoProfile -ExecutionPolicy Bypass -File \"$remote\\cleanup.ps1\"" |
        sed 's/\r$//; s/^/  /' >&2 || true
    on_tv "schtasks /delete /tn $task /f >nul 2>&1" || true
    if ((keep)); then
        log "kept the test's folder on $host: $remote; remove it with: ssh $host rmdir /s /q \"$remote\""
    else
        # A process just ended can hold its files for a moment: tried three times, a second apart (ping is the
        # wait: timeout needs a console, which this ssh has not).
        for _ in 1 2 3; do
            tv_says "rmdir /s /q \"$remote\" 2>nul & if exist \"$remote\" echo there" there || break
            on_tv "ping -n 2 127.0.0.1 >nul" || true
        done
        if tv_says "if exist \"$remote\" echo there" there; then
            log "could not remove $remote on $host: remove it with: ssh $host rmdir /s /q \"$remote\""
        fi
    fi
    rm -rf "$stage"
    exit "$code"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

on_tv "if exist \"$remote\" rmdir /s /q \"$remote\"" || true
# A folder left by an earlier run that Windows will not remove (its dashboard.exe still held open by something that
# watched the process) must not stop this run: it goes into a folder of its own beside it.
if tv_says "if exist \"$remote\" echo there" there; then
    again="$test.$(date +%H%M%S)"
    log "an earlier run's folder cannot be removed ($remote); using xsd-e2e\\$again"
    remote="%TEMP%\\xsd-e2e\\$again"
    remote_scp="AppData/Local/Temp/xsd-e2e/$again"
fi
on_tv "mkdir \"$remote\"" >/dev/null || true
copy "$stage"/* "$host:$remote_scp/" || { echo "could not copy the test to $host:$remote_scp" >&2; exit 2; }

# A one-off task in the signed-in user's session (/it), started now; its time is never reached.
tv_says "schtasks /create /tn $task /tr \"\\\"$remote\\run.cmd\\\"\" /sc once /st 00:00 /it /f >nul && schtasks /run /tn $task >nul && echo started" started ||
    { echo "could not start the test's task on $host" >&2; exit 2; }
log "started $test on $host; waiting up to $minutes min"

code=""
deadline=$((SECONDS + minutes * 60))
while ((SECONDS < deadline)); do
    sleep 5
    code="$(on_tv "if exist \"$remote\\out\\exitcode.txt\" type \"$remote\\out\\exitcode.txt\"" | tr -dc '0-9')" || code=""
    [[ -n "$code" ]] && break
done

copy "$host:$remote_scp/out/*" "$out/" || log "could not copy the test's output back"
[[ -f "$out/refused.txt" ]] && sed 's/\r$//' "$out/refused.txt" >&2
[[ -f "$out/steps.txt" ]] && sed 's/\r$//' "$out/steps.txt" >&2
if [[ -z "$code" ]]; then
    log "TIMEOUT: $test gave no exit code in $minutes min ($out)"
    exit 124
fi
case "$code" in
    0) log "PASS: $test ($out)" ;;
    3) log "NOT RUN: the TV is in use ($out)" ;;
    *) log "FAIL: $test, exit $code ($out)" ;;
esac
exit "$code"
