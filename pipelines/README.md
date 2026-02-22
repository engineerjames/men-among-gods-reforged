# Pipelines

This folder contains small scripts used by GitHub Actions workflows (and runnable locally) for building, packaging, and publishing releases.

## Scripts

- `install_linux_deps.sh`
  - Installs Linux build dependencies for Bevy and SDL2 (intended for `ubuntu-latest`).
- `build_release.sh`
  - Builds release binaries for `server` and `sdl_client`.
- `package.sh`
  - Creates release `.zip` packages on Linux/macOS.
  - Produces `dist/men-among-gods-{server,client}-<version>-<platform>.zip`.
- `package_windows.ps1`
  - Creates release `.zip` packages on Windows.
  - Produces `dist/men-among-gods-{server,client}-<version>-<platform>.zip`.
- `create_github_release.sh`
  - Creates (or updates) a GitHub release and uploads `.zip` assets from an artifacts directory.

## Prerequisites

SDL2 is managed automatically via [cargo-vcpkg](https://crates.io/crates/cargo-vcpkg) on all platforms (Windows, Linux, macOS). No system SDL2 installation is required.

```bash
cargo install cargo-vcpkg
cargo vcpkg build --manifest-path sdl_client/Cargo.toml
```

This downloads and builds the SDL2 libraries once and caches them; subsequent builds reuse the cache. Linux builds also need Bevy dependencies for the legacy client:

```bash
bash pipelines/install_linux_deps.sh
```

## Local usage

### Build release binaries

```bash
bash pipelines/build_release.sh
```

### Package on macOS/Linux

```bash
bash pipelines/package.sh --version v0.1.0 --platform macos
# or
bash pipelines/package.sh --version v0.1.0 --platform linux
```

### Package on Windows

```powershell
pwsh -File pipelines/package_windows.ps1 -Version v0.1.0 -Platform windows
```

### Create/update a GitHub release (upload assets)

Requirements:
- `gh` installed
- `GH_TOKEN` set (a token with `repo` scope for private repos or appropriate permissions)
- `GH_REPO` set to `owner/repo`

Example:

```bash
export GH_REPO="owner/repo"
export GH_TOKEN="..."

bash pipelines/create_github_release.sh --version v0.1.0 --artifacts-dir dist
```

In GitHub Actions, `GH_REPO` and `GH_TOKEN` are provided by the workflow.
