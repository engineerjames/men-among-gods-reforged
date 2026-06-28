# Pipelines

This folder contains small scripts used by GitHub Actions workflows, and runnable locally, for building and packaging releases.

## Scripts

- `install_linux_deps.sh`
  - Installs Linux build dependencies for SDL2 (intended for `ubuntu-latest`).
- `install_macos_deps.sh`
  - Installs macOS build dependencies for SDL2 (intended for `macos-latest`).
- `build_release.sh`
  - Builds release binaries for the workspace.
- `package.sh`
  - Creates release `.zip` packages on Linux/macOS.
  - Produces `dist/men-among-gods-{server,client}-<version>-<platform>.zip`.
  - The server archive includes `server`, `api`, `map_viewer`, `template_viewer`, `world-snapshot`, and `assets/world_seed.wsnap`.
- `package_windows.ps1`
  - Creates release `.zip` packages on Windows.
  - Produces `dist/men-among-gods-{server,client}-<version>-<platform>.zip`.

GitHub release creation and artifact upload are handled directly in [.github/workflows/release.yml](../.github/workflows/release.yml).

## Prerequisites

SDL2 is managed automatically via [cargo-vcpkg](https://crates.io/crates/cargo-vcpkg) on all platforms (Windows, Linux, macOS). No system SDL2 installation is required.

```bash
cargo install cargo-vcpkg
cargo vcpkg -v build --manifest-path client/Cargo.toml
```

This downloads and builds the SDL2 libraries once and caches them; subsequent builds reuse the cache. Linux builds also need system headers:

```bash
bash pipelines/install_linux_deps.sh
```

## Local usage

### Update product version before release

Edit the `version` field in the root `Cargo.toml` `[workspace.package]` section, run verification tests, then commit the version bump before dispatching the GitHub release workflow.

```toml
[workspace.package]
version = "1.2.1"
```

Then verify and commit:

```bash
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
git add Cargo.toml && git commit -m "Bump version to 1.2.1"
```

Use the bare semver version (`1.2.1`) as the Cargo product version; the GitHub release workflow will use this to create the release tag (`v1.2.1`) for packaging.
Protocol compatibility values in `core::constants::VERSION` and `core::constants::MINVERSION` are intentionally separate from routine product release bumps.

### Build release binaries

```bash
bash pipelines/build_release.sh
```

### Package on macOS/Linux

```bash
bash pipelines/build_release.sh
bash pipelines/package.sh --version v0.1.0 --platform macos
# or
bash pipelines/build_release.sh
bash pipelines/package.sh --version v0.1.0 --platform linux
```

### Package on Windows

```powershell
bash pipelines/build_release.sh
pwsh -File pipelines/package_windows.ps1 -Version v0.1.0 -Platform windows
```

The package scripts expect the release build outputs to already exist and fail fast if any required binary or the bundled snapshot seed file is missing.
