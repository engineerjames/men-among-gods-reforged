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

### Steamworks IDs

| Item | ID |
|---|---|
| App | `4665990` |
| Depot: Windows | `4665991` |
| Depot: macOS | `4665992` |
| Depot: Linux | `4665993` |
| Package: Beta Testing (release override, use for friend/tester keys) | `1624405` |
| Package: Developer Comp | `1624404` |
| Package: Free on demand (public F2P, keep **Disabled** until launch) | `1624406` |
| Package: CD Key | `1624407` |

### One-time setup

1. In Steamworks (App Admin → Edit Steamworks Settings):
   - **SteamPipe → Depots**: one depot per platform with its Operating System set, then **Save**.
   - Every package above must list all three depots under **Depots Included** (open the package from App Admin → All Associated Packages, DLC, Demos and Tools → **Add/Remove Depots**).
   - **Installation → General Installation**: set the install folder and one launch option per OS:
     - Windows: `men-among-gods-client.exe`
     - macOS: `Men Among Gods - Reforged.app`
     - Linux: `men-among-gods-client`
   - **Publish → Prepare for Publishing → Publish to Steam**. Configuration changes do nothing until published.
2. Create a dedicated Steam builder account with Steam Guard enabled. Invite it under Users & Permissions → Manage Users and grant it "Edit App Metadata" and "Publish App Changes To Steam" for the app.
3. Set the GitHub Actions variables (requires admin on the repo; or use Settings → Secrets and variables → Actions → Variables):

   ```bash
   gh variable set STEAM_APP_ID --body 4665990
   gh variable set STEAM_DEPOT_WINDOWS --body 4665991
   gh variable set STEAM_DEPOT_MACOS --body 4665992
   gh variable set STEAM_DEPOT_LINUX --body 4665993   # omit to skip Linux
   gh variable list
   ```

4. Cache the builder's credentials locally, then store them as secrets. Run steamcmd with an isolated `HOME`: on macOS steamcmd otherwise shares `~/Library/Application Support/Steam` with the desktop Steam client, whose `config.vdf` contains your personal account's login cache and must not be uploaded. The secret is gzipped because the raw file can exceed GitHub's 48 KB secret limit.

   ```bash
   brew install --cask steamcmd
   BUILDER_HOME=~/.mag-steam-builder
   mkdir -p "$BUILDER_HOME"
   HOME="$BUILDER_HOME" steamcmd +login <builder_username> +quit   # enter password + Steam Guard code
   gh secret set STEAM_USERNAME --body <builder_username>
   gzip -9c "$BUILDER_HOME/Library/Application Support/Steam/config/config.vdf" | base64 | gh secret set STEAM_CONFIG_VDF
   ```

If the upload fails to log in, the cached credentials have expired; repeat step 4.

### Uploading a build

```bash
gh workflow run release.yml -f version=v0.1.0 -f steam_upload=true
```

### Going live

Steam does not allow build scripts to set the `default` branch live. Either set the build live from Steamworks (SteamPipe → Builds → set live on `default` → Preview Change → Set Build Live Now), or pass a beta branch name in the workflow's **steam_branch** input.

### Private beta access

While the app is unreleased (do **not** press Release or set the store page to "Coming Soon"), only key holders can see and install it. Request keys for the **Beta Testing** package (`1624405`) via App Admin → Request Steam Product Keys; testers redeem them in Steam via Games → Activate a Product on Steam.

### Local usage

Unzip the client packages, then:

```bash
bash pipelines/steam_upload.sh --app-id 4665990 --description v0.1.0 \
  --depot 4665991=dist/men-among-gods-client-v0.1.0-windows \
  --depot 4665992=dist/men-among-gods-client-v0.1.0-macos \
  --username <builder_username>
```

On macOS, prefix the command with `HOME=~/.mag-steam-builder` so steamcmd uses the builder's cached login (see step 4).

Add `--dry-run` to print the generated SteamPipe build VDF without uploading.
