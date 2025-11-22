# Windows Build Issue - aws-lc-rs Dependency

## Problem

Windows builds are failing in GitHub Actions because `aws-lc-rs` (a cryptographic library used by `rustls`) requires:
1. CMake
2. NASM assembler  
3. Working C/C++ build environment
4. Causes issues with long Windows file paths

Error:
```
error: failed to run custom build command for `aws-lc-sys v0.32.3`
Missing dependency: cmake
NASM command not found or failed to execute.
```

## Root Cause

Dependency chain:
```
android-adb-run 
  → adb_client
    → rustls (v0.23)
      → aws-lc-rs (default crypto provider)
        → aws-lc-sys (C library, needs CMake/NASM)
```

`rustls` v0.23 defaults to using `aws-lc-rs` as its crypto provider, but this has build issues on Windows.

## Solution Attempts

### Attempt 1: Patch crates.io ❌
Tried to patch `rustls` from crates.io with custom features - doesn't work because you can't patch a crate from crates.io with itself.

### Attempt 2: Feature flags to disable TCP ❌  
Made `adb_client` optional behind a feature flag - too invasive, requires rewriting large parts of the codebase.

### Attempt 3: Target-specific dependencies ❌
Added `rustls` with `ring` feature for Windows target:
```toml
[target.'cfg(windows)'.dependencies]
rustls = { version = "0.23", default-features = false, features = ["ring"] }
```
Doesn't work - doesn't override transitive dependencies.

## Working Solution

The only reliable solution for Windows builds is to use the **GNU toolchain** instead of MSVC, or install the required build dependencies.

### Option A: Install Build Dependencies in CI (Recommended)

Add to Windows build step in `.github/workflows/ci.yml`:

```yaml
- name: Install Windows build dependencies
  if: runner.os == 'Windows'
  run: |
    choco install cmake nasm -y
```

###Option B: Use Pre-built Binaries

The `aws-lc-rs` crate can use pre-built binaries if available. Set environment variable:

```yaml
- name: Build release binary
  if: runner.os == 'Windows'
  working-directory: android-adb-run
  env:
    AWS_LC_SYS_PREBUILT_NASM: "1"
  run: cargo build --release --target ${{ matrix.target }}
```

### Option C: Force Ring Crypto Backend

Use a Cargo.toml workspace-level override (requires workspace):

```toml
[workspace]
members = ["android-adb-run"]

[workspace.dependencies]
rustls = { version = "0.23", default-features = false, features = ["ring", "std", "tls12"] }
```

Then reference it:
```toml
[dependencies]
rustls = { workspace = true }
```

### Option D: Fork adb_client

Fork `adb_client` and modify it to use `ring` instead of default features. Most invasive but most reliable.

## Recommended Approach

**Install CMake and NASM in GitHub Actions** (Option A):

1. Update `.github/workflows/ci.yml` and `.github/workflows/release.yml`
2. Add chocolatey install step for Windows
3. Keep current Cargo.toml structure

This is the cleanest solution that doesn't require code changes.

## Current State

- Added `[target.'cfg(windows)'.dependencies]` to Cargo.toml (doesn't fully work)
- Updated workflows to use standard build commands
- Reverted feature flag changes to keep code simple

## Next Steps

1. Add CMake/NASM installation to Windows CI jobs
2. Test Windows build completes successfully
3. Document Windows development requirements in README

## References

- aws-lc-rs docs: https://aws.github.io/aws-lc-rs/
- rustls crypto providers: https://docs.rs/rustls/latest/rustls/#cryptography-providers
- GitHub Actions Windows: https://github.com/actions/runner-images/blob/main/images/windows/Windows2022-Readme.md
