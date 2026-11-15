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
- `steam_upload.sh`
  - Uploads unpacked client packages to Steam with SteamPipe (`steamcmd`). See [Steam uploads](#steam-uploads).

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

### Product version

The GitHub release workflow stamps the dispatched tag (e.g. `v1.2.1` → `1.2.1`) into the root `Cargo.toml` `[workspace.package]` `version` before building, so the client window title, server/API startup logs, and macOS bundle all report the release version. The tag must be semver (optionally `v`-prefixed); the workflow fails otherwise.

Committing the bump is optional but keeps local builds in sync. To do so, edit the `version` field in the root `Cargo.toml` `[workspace.package]` section, run verification tests, then commit.

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

## Steam uploads

The client is distributed on Steam without the Steamworks SDK (accounts are handled by our own API). The release workflow can upload the client packages to SteamPipe when dispatched with **steam_upload** checked.

### One-time setup

1. In Steamworks, create one depot per platform (App Admin → SteamPipe → Depots), set each depot's OS, add the depots to the store package, and configure a launch option per OS:
   - Windows: `men-among-gods-client.exe`
   - macOS: `Men Among Gods - Reforged.app`
   - Linux: `men-among-gods-client`

   Then **Publish** the app configuration (Publish tab).
2. Create a dedicated Steam builder account with Steam Guard enabled and grant it "Edit App Metadata" and "Publish App Changes To Steam" for the app.
3. Cache the builder's credentials locally and export them:

   ```bash
   steamcmd +login <builder_username> +quit   # enter password + Steam Guard code
   base64 < ~/Library/Application\ Support/Steam/config/config.vdf   # macOS
   base64 -w0 < ~/Steam/config/config.vdf                             # Linux (path may vary)
   ```

4. In GitHub (Settings → Secrets and variables → Actions) add:
   - Secrets: `STEAM_USERNAME`, `STEAM_CONFIG_VDF` (the base64 output above).
   - Variables: `STEAM_APP_ID`, and `STEAM_DEPOT_WINDOWS` / `STEAM_DEPOT_MACOS` / `STEAM_DEPOT_LINUX`. Leave a depot variable unset to skip that platform.

If the upload fails to log in, the cached credentials have expired; repeat step 3 and update `STEAM_CONFIG_VDF`.

### Going live

Steam does not allow build scripts to set the `default` branch live. Either set the build live from Steamworks (App Admin → SteamPipe → Builds), or pass a beta branch name in the workflow's **steam_branch** input. While the app is unreleased, only accounts holding a key (Request Steam Product Keys) can see and install it.

### Local usage

Unzip the client packages, then:

```bash
bash pipelines/steam_upload.sh --app-id 1234560 --description v0.1.0 \
  --depot 1234561=dist/men-among-gods-client-v0.1.0-windows \
  --depot 1234562=dist/men-among-gods-client-v0.1.0-macos \
  --username <builder_username>
```

Add `--dry-run` to print the generated SteamPipe build VDF without uploading.
