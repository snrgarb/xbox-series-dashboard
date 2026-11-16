# shellcheck shell=bash disable=SC2154  # $here and $repo are the caller's
# Sourced by e2e/run.sh (the VM) and e2e/htpc/run.sh (the TV PC): puts one test's payload together in a folder, as
# both run it on Windows:
#
#   <stage>\run.ps1 and the rest of payloads/<test>/
#   <stage>\lib\E2E.psm1, lib\Nefarius.ViGEm.Client.dll
#   <stage>\dashboard.exe, standin.exe, and whatever other .exe was built beside them (the spikes')
#
#   stage_payload <test> <dashboard.exe> <standin.exe> <stage dir>
#
# Needs $here (e2e/) and $repo set by the caller. Exits 2 for a test or a program that is not there.

stage_payload() {
    local test="$1" exe="$2" standin="$3" stage="$4"
    local payload_src="$here/payloads/$test"
    [[ -f "$payload_src/run.ps1" ]] || { echo "no test $test (no $payload_src/run.ps1)" >&2; exit 2; }
    for built_exe in "$exe" "$standin"; do
        [[ -f "$built_exe" ]] && continue
        echo "no $built_exe" >&2
        echo "build it first: cargo build --release --target x86_64-pc-windows-gnullvm -j 6" >&2
        exit 2
    done

    cp -r "$payload_src/." "$stage/"
    cp -r "$here/payloads/lib" "$stage/lib"
    # Whatever else was built goes too: the spikes' exes, for the payloads that run them.
    cp "$repo/target/x86_64-pc-windows-gnullvm/release"/*.exe "$stage/" 2>/dev/null || true
    # And whatever was built beside the Dashboard under test (CI's build, downloaded), which wins over a local build.
    cp "$(dirname "$exe")"/*.exe "$stage/" 2>/dev/null || true
    cp "$exe" "$stage/dashboard.exe"
    cp "$standin" "$stage/standin.exe"

    # The ViGEm client the tests drive their virtual pad with: managed, netstandard2.0, loaded by PowerShell 5.1.
    # Fetched once from NuGet, pinned by version and checksum, and kept under artifacts/e2e/.cache.
    local vigem_version="1.21.256"
    local vigem_sha256="4458301000b732d115521e99f9936f4edb70d6ceb3036ef158715e0e6b8902e0"
    local vigem="$repo/artifacts/e2e/.cache/Nefarius.ViGEm.Client-$vigem_version.dll" nupkg
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
}
