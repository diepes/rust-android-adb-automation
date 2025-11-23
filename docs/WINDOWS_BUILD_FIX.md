# Windows Build Issue - aws-lc-rs Dependency

## Problem

Windows builds fail in GitHub Actions when the `aws-lc-sys` build script tries to
compile its `c11.c` check with MSVC. Newer Microsoft CRT headers require C11
atomics support, and MSVC only enables them when the compiler flag `/std:c11`
is passed. Without that flag the build aborts with:

```text
C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Tools\MSVC\14.44.35207\include\vcruntime_c11_stdatomic.h(16): fatal error C1189: "C atomics require C11 or later"
```

## Root Cause

Dependency chain:

```text
android-adb-run 
  → adb_client
    → rustls (v0.23)
      → aws-lc-rs (default crypto provider)
        → aws-lc-sys (C library, needs CMake/NASM)
```

`rustls` v0.23 defaults to using `aws-lc-rs` as its crypto provider, but this has build issues on Windows.

## Solution Attempts

### Attempt 1: Switch rustls backend ❌

Tried to force the RustTLS ecosystem to use the pure-Rust `ring` backend.
Unfortunately `adb_client` enables the `aws-lc-rs` feature internally, so the
transitive dependency stays enabled anyway.

### Attempt 2: Disable TCP stack ❌

Considered hiding `adb_client` behind a feature flag so the GUI would ship
without network ADB support on Windows. This would remove the dependency but
breaks functionality and forks the code paths.

### Attempt 3: Target-specific dependency override ❌

Added a Windows-only dependency override for `rustls` so it would use `ring`.
Cargo feature resolution is additive, so the default features requested by
`adb_client` still pulled in `aws-lc-rs`.

## Working Solution

We keep using the official MSVC toolchain, install the two required build
dependencies (CMake + NASM), and set extra compiler flags when building on
Windows so that MSVC enables its C11 mode.

### Workflow changes

Both CI workflows now contain these platform-specific steps:

```yaml
- name: Install Windows build dependencies
  if: runner.os == 'Windows'
  run: |
    choco install cmake nasm -y

- name: Build release binary (Windows)
  if: runner.os == 'Windows'
  env:
    CFLAGS_x86_64_pc_windows_msvc: "/std:c11 /experimental:c11atomics"
    CXXFLAGS_x86_64_pc_windows_msvc: "/std:c++17"
  run: cargo build --release --target ${{ matrix.target }}
```

Setting `CFLAGS_x86_64_pc_windows_msvc` to include `/std:c11` enables the
modern CRT headers, and `/experimental:c11atomics` unlocks the atomics the
headers expect. `CXXFLAGS_x86_64_pc_windows_msvc=/std:c++17`
matches the expectations of the C++ sources that come with the library.

## Current State

- Windows builds install CMake and NASM automatically.
- Cargo is instructed to compile C code with `/std:c11`, unblocking the
  `aws-lc-sys` build script.
- Linux and macOS builds continue unchanged.

## Next Steps

1. Let GitHub Actions finish a full run to verify the fix.
2. Keep an eye on future `aws-lc-rs` releases; if they switch defaults again we
   may revisit forcing the ring backend.
3. Document the MSVC C11 requirement for anyone building locally on Windows.

## Troubleshooting CI Logs

When the GitHub CLI truncates large logs, fetch the job output directly from
the REST API and inspect it locally:

```bash
# Download the raw log for a given job ID
gh api repos/<owner>/<repo>/actions/jobs/<job_id>/logs > job.log

# Quickly highlight failing portions
rg "error:" job.log | head
```

The helper script `check-gh-build.sh` also sets `GH_DEBUG=api` so every `gh`
invocation prints the underlying HTTP requests, which is handy when diagnosing
authentication or pagination issues.

## References

- [aws-lc-rs docs](https://aws.github.io/aws-lc-rs/)
- [rustls crypto providers](https://docs.rs/rustls/latest/rustls/#cryptography-providers)
- [GitHub Actions Windows image reference](https://github.com/actions/runner-images/blob/main/images/windows/Windows2022-Readme.md)
