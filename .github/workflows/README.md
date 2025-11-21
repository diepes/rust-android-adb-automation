# GitHub Actions Workflows

This directory contains GitHub Actions workflows for CI/CD.

## Workflows

### 1. CI Workflow (`ci.yml`)

**Trigger**: Runs on every push and pull request to `main`, `master`, or `develop` branches.

**Jobs**:
- **Test**: Runs on Linux, macOS, and Windows
  - Checks code compilation (`cargo check`)
  - Runs all tests (`cargo test`)
  - Runs linter (`cargo clippy`)
  
- **Build**: Builds release binaries for all platforms
  - Linux (x86_64)
  - macOS (x86_64 and aarch64/Apple Silicon)
  - Windows (x86_64)
  - Uploads artifacts for download

**Features**:
- Caching for faster builds
- Cross-platform testing
- Artifact uploads for each platform

### 2. Release Workflow (`release.yml`)

**Trigger**: Runs when you push a git tag matching `v*.*.*` (e.g., `v1.0.0`)

**Jobs**:
- Creates a GitHub Release
- Builds release binaries for all platforms
- Creates compressed archives (`.tar.gz` for Unix, `.zip` for Windows)
- Uploads binaries to the GitHub Release

**How to create a release**:
```bash
# Create and push a version tag
git tag v1.0.0
git push origin v1.0.0
```

This will automatically:
1. Create a GitHub Release named "Release v1.0.0"
2. Build binaries for all platforms
3. Attach the binaries to the release

## Platform-Specific Dependencies

### Linux
The workflow automatically installs these dependencies on Ubuntu:
- `libwebkit2gtk-4.1-dev` - For Dioxus GUI
- `libgtk-3-dev` - GTK3 development files
- `libayatana-appindicator3-dev` - System tray support
- `librsvg2-dev` - SVG rendering
- `patchelf` - Binary patching utility
- `libssl-dev` - OpenSSL development files
- `pkg-config` - Package configuration

### macOS
No additional dependencies needed - uses system frameworks.

### Windows
No additional dependencies needed - uses MSVC toolchain.

## Build Artifacts

After a successful CI build, you can download the artifacts:

1. Go to the "Actions" tab in your GitHub repository
2. Click on the latest workflow run
3. Scroll down to "Artifacts"
4. Download the binary for your platform:
   - `android-adb-run-linux-x86_64`
   - `android-adb-run-macos-x86_64`
   - `android-adb-run-macos-aarch64` (Apple Silicon)
   - `android-adb-run-windows-x86_64.exe`

## Local Testing

To test if your code will pass CI before pushing:

```bash
cd android-adb-run

# Check compilation
cargo check

# Run tests
cargo test

# Run linter
cargo clippy -- -D warnings

# Build release
cargo build --release
```

## Troubleshooting

### Linux Build Fails
- Make sure all system dependencies are installed
- Check if `libwebkit2gtk-4.1-dev` is available (may need `libwebkit2gtk-4.0-dev` on older Ubuntu)

### macOS Build Fails
- Ensure you're targeting the correct architecture
- For Apple Silicon: `cargo build --target aarch64-apple-darwin`
- For Intel: `cargo build --target x86_64-apple-darwin`

### Windows Build Fails
- Ensure MSVC toolchain is installed
- May need to install Visual Studio Build Tools

### Clippy Warnings
- Currently set to `continue-on-error: true` so warnings don't fail the build
- Remove this to make warnings fail the CI

## Customization

### Change Trigger Branches
Edit the `on.push.branches` and `on.pull_request.branches` in `ci.yml`:
```yaml
on:
  push:
    branches: [ main, develop, feature/* ]
```

### Add More Targets
Add to the `matrix.include` section:
```yaml
- os: ubuntu-latest
  target: aarch64-unknown-linux-gnu
  artifact_name: android-adb-run
  asset_name: android-adb-run-linux-aarch64
```

### Skip Tests
Remove or comment out the test job if you don't have tests yet.

## Performance

**Caching**: The workflows cache:
- Cargo registry
- Cargo git index
- Target directory

This significantly speeds up subsequent builds.

**Build Times** (approximate):
- First build: 10-15 minutes per platform
- Cached builds: 3-5 minutes per platform
