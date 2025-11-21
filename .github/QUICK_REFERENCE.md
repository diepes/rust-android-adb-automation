# Quick Reference: GitHub Actions Commands

## First Time Setup

```bash
# Add the new workflow files
git add .github/

# Commit the changes
git commit -m "Add GitHub Actions CI/CD workflows"

# Push to GitHub
git push origin main
```

Then go to your repository on GitHub and click the "Actions" tab to see your first build!

---

## Regular Development Workflow

```bash
# 1. Make your changes
# ... edit files ...

# 2. Test locally (optional but recommended)
cd android-adb-run
cargo test
cargo clippy

# 3. Commit and push
git add .
git commit -m "Your commit message"
git push origin main

# 4. CI will automatically run!
# Check the "Actions" tab on GitHub
```

---

## Creating a Release

```bash
# 1. Update version in Cargo.toml
# version = "1.0.0"

# 2. Commit version bump
git add android-adb-run/Cargo.toml
git commit -m "Bump version to 1.0.0"

# 3. Create and push tag
git tag v1.0.0
git push origin main
git push origin v1.0.0

# 4. GitHub Actions will:
#    - Create a release on GitHub
#    - Build binaries for all platforms
#    - Upload them to the release
```

---

## Viewing Build Results

### CI Build Artifacts
1. Go to: `https://github.com/YOUR_USERNAME/YOUR_REPO/actions`
2. Click on the latest workflow run
3. Scroll down to "Artifacts"
4. Download the binary for your platform

### Release Binaries
1. Go to: `https://github.com/YOUR_USERNAME/YOUR_REPO/releases`
2. Click on the latest release
3. Download the binary from "Assets"

---

## Troubleshooting

### Build failing? Test locally first:
```bash
cd android-adb-run

# Check compilation
cargo check

# Run tests
cargo test

# Check for warnings
cargo clippy -- -D warnings

# Build release
cargo build --release
```

### Need to delete a tag?
```bash
# Delete local tag
git tag -d v1.0.0

# Delete remote tag
git push origin :refs/tags/v1.0.0
```

### Want to see workflow logs?
1. Go to "Actions" tab
2. Click on the failed workflow
3. Click on the failing job
4. Expand the failing step to see logs

---

## Status Badges

Add this to your README.md to show build status:

```markdown
![CI](https://github.com/YOUR_USERNAME/YOUR_REPO/workflows/CI/badge.svg)
```

Replace `YOUR_USERNAME` and `YOUR_REPO` with your actual values.

---

## Version Tags Convention

Use semantic versioning:
- `v1.0.0` - Major release (breaking changes)
- `v1.1.0` - Minor release (new features)
- `v1.1.1` - Patch release (bug fixes)

Examples:
```bash
git tag v0.1.0  # First alpha release
git tag v1.0.0  # First stable release
git tag v1.0.1  # Bug fix
git tag v1.1.0  # New feature
git tag v2.0.0  # Breaking changes
```
