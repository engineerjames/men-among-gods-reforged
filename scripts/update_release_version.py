#!/usr/bin/env python3
"""Update the workspace product version before cutting a release."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SEMVER_RE = re.compile(r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$")
EXPECTED_WORKSPACE_PACKAGES = {
    "api",
    "client",
    "client-utils",
    "core",
    "server",
    "server-utils",
}
STALE_NESTED_LOCKFILES = (Path("client/utils/Cargo.lock"),)


class ReleaseVersionError(RuntimeError):
    """Raised when the release version update cannot be completed safely."""


def parse_release_version(value: str) -> str:
    """Return a strict bare semver release version."""
    if not SEMVER_RE.fullmatch(value):
        raise ReleaseVersionError(
            "Version must be bare semver in X.Y.Z form, for example 1.2.3"
        )
    return value


def release_tag(version: str) -> str:
    """Return the GitHub release tag derived from a bare semver version."""
    return f"v{version}"


def run_command(
    args: list[str], cwd: Path, capture_output: bool = True
) -> subprocess.CompletedProcess[str]:
    """Run a command and return its completed process."""
    return subprocess.run(
        args,
        cwd=cwd,
        check=False,
        text=True,
        stdout=subprocess.PIPE if capture_output else None,
        stderr=subprocess.PIPE if capture_output else None,
    )


def ensure_clean_worktree(root: Path, allow_dirty: bool) -> None:
    """Refuse to run when the Git worktree already has changes."""
    if allow_dirty:
        return

    git_check = run_command(["git", "rev-parse", "--is-inside-work-tree"], root)
    if git_check.returncode != 0:
        return

    status = run_command(["git", "status", "--porcelain"], root)
    if status.returncode != 0:
        raise ReleaseVersionError(status.stderr.strip() or "Unable to inspect Git status")

    changed = [line for line in status.stdout.splitlines() if line]
    if changed:
        preview = "\n".join(changed[:12])
        extra = "" if len(changed) <= 12 else f"\n...and {len(changed) - 12} more"
        raise ReleaseVersionError(
            "Working tree has changes. Commit/stash them or pass --allow-dirty.\n"
            f"{preview}{extra}"
        )


def replace_workspace_version_text(contents: str, version: str) -> str:
    """Replace the version inside the root [workspace.package] section."""
    lines = contents.splitlines(keepends=True)
    in_workspace_package = False
    found_section = False
    replacements = 0

    for index, line in enumerate(lines):
        stripped = line.strip()

        if stripped == "[workspace.package]":
            in_workspace_package = True
            found_section = True
            continue

        if in_workspace_package and stripped.startswith("[") and stripped.endswith("]"):
            in_workspace_package = False

        if not in_workspace_package:
            continue

        if re.match(r"^\s*version\s*=", line):
            line_ending = (
                "\r\n" if line.endswith("\r\n") else "\n" if line.endswith("\n") else ""
            )
            indent = re.match(r"^\s*", line).group(0)
            lines[index] = f'{indent}version = "{version}"{line_ending}'
            replacements += 1

    if not found_section:
        raise ReleaseVersionError("Root Cargo.toml is missing [workspace.package]")
    if replacements != 1:
        raise ReleaseVersionError(
            f"Expected exactly one workspace package version, found {replacements}"
        )

    return "".join(lines)


def update_workspace_manifest(root: Path, version: str, dry_run: bool) -> bool:
    """Update root Cargo.toml and return whether its contents changed."""
    cargo_toml = root / "Cargo.toml"
    contents = cargo_toml.read_text(encoding="utf-8")
    updated = replace_workspace_version_text(contents, version)
    changed = updated != contents

    if changed and not dry_run:
        cargo_toml.write_text(updated, encoding="utf-8")

    return changed


def replace_lockfile_workspace_versions_text(
    contents: str, version: str, package_names: set[str]
) -> tuple[str, set[str]]:
    """Replace local workspace package versions in Cargo.lock text."""
    lines = contents.splitlines(keepends=True)
    updated_package_names: set[str] = set()

    package_name: str | None = None
    package_version_index: int | None = None
    package_has_source = False

    def finish_package() -> None:
        nonlocal package_name, package_version_index, package_has_source

        if (
            package_name in package_names
            and package_version_index is not None
            and not package_has_source
        ):
            old_line = lines[package_version_index]
            line_ending = (
                "\r\n"
                if old_line.endswith("\r\n")
                else "\n" if old_line.endswith("\n") else ""
            )
            indent = re.match(r"^\s*", old_line).group(0)
            lines[package_version_index] = f'{indent}version = "{version}"{line_ending}'
            updated_package_names.add(package_name)

        package_name = None
        package_version_index = None
        package_has_source = False

    for index, line in enumerate(lines):
        stripped = line.strip()
        if stripped == "[[package]]":
            finish_package()
            continue

        if re.match(r"^\s*name\s*=", line):
            name_match = re.match(r'^\s*name\s*=\s*"([^"]+)"', line)
            if name_match:
                package_name = name_match.group(1)
            continue

        if re.match(r"^\s*version\s*=", line):
            package_version_index = index
            continue

        if re.match(r"^\s*source\s*=", line):
            package_has_source = True

    finish_package()

    missing = package_names.difference(updated_package_names)
    if missing:
        raise ReleaseVersionError(
            "Cargo.lock is missing local workspace packages: " + ", ".join(sorted(missing))
        )

    return "".join(lines), updated_package_names


def update_lockfile_workspace_versions(root: Path, version: str, dry_run: bool) -> set[str]:
    """Update local workspace package versions in the root Cargo.lock."""
    lockfile = root / "Cargo.lock"
    contents = lockfile.read_text(encoding="utf-8")
    updated, package_names = replace_lockfile_workspace_versions_text(
        contents, version, EXPECTED_WORKSPACE_PACKAGES
    )

    if updated != contents and not dry_run:
        lockfile.write_text(updated, encoding="utf-8")

    return package_names


def remove_stale_nested_lockfiles(root: Path, dry_run: bool) -> list[Path]:
    """Remove lockfiles that belong to crates now covered by the workspace lockfile."""
    removed: list[Path] = []
    for relative_path in STALE_NESTED_LOCKFILES:
        lockfile = root / relative_path
        if lockfile.exists():
            removed.append(relative_path)
            if not dry_run:
                lockfile.unlink()
    return removed


def workspace_versions_from_metadata(metadata: dict[str, object]) -> dict[str, str]:
    """Extract workspace package versions from cargo metadata JSON."""
    workspace_members = set(metadata.get("workspace_members", []))
    packages = metadata.get("packages", [])
    versions: dict[str, str] = {}

    for package in packages:
        if not isinstance(package, dict):
            continue
        package_id = package.get("id")
        if package_id not in workspace_members:
            continue
        name = package.get("name")
        version = package.get("version")
        if isinstance(name, str) and isinstance(version, str):
            versions[name] = version

    return versions


def read_workspace_versions(root: Path) -> dict[str, str]:
    """Read local workspace package versions from cargo metadata."""
    metadata = run_command(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        root,
    )
    if metadata.returncode != 0:
        raise ReleaseVersionError(metadata.stderr.strip() or "cargo metadata failed")

    return workspace_versions_from_metadata(json.loads(metadata.stdout))


def verify_workspace_versions(versions: dict[str, str], expected_version: str) -> None:
    """Assert every expected workspace package reports the requested version."""
    missing = sorted(EXPECTED_WORKSPACE_PACKAGES.difference(versions))
    mismatched = {
        name: version
        for name, version in sorted(versions.items())
        if name in EXPECTED_WORKSPACE_PACKAGES and version != expected_version
    }

    if missing or mismatched:
        details: list[str] = []
        if missing:
            details.append("missing packages: " + ", ".join(missing))
        if mismatched:
            details.append(
                "mismatched versions: "
                + ", ".join(f"{name}={version}" for name, version in mismatched.items())
            )
        raise ReleaseVersionError("Workspace version verification failed: " + "; ".join(details))


def parse_args(argv: list[str]) -> argparse.Namespace:
    """Parse command-line arguments."""
    parser = argparse.ArgumentParser(
        description="Update Men Among Gods - Reforged product release versions."
    )
    parser.add_argument("--version", required=True, help="Bare release version, for example 1.2.3")
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Show intended changes without writing files or updating lockfiles",
    )
    parser.add_argument(
        "--allow-dirty",
        action="store_true",
        help="Allow running when the Git working tree already has changes",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    """Run the release version update."""
    args = parse_args(sys.argv[1:] if argv is None else argv)

    try:
        version = parse_release_version(args.version)
        tag = release_tag(version)
        ensure_clean_worktree(REPO_ROOT, args.allow_dirty)

        manifest_changed = update_workspace_manifest(REPO_ROOT, version, args.dry_run)
        removed_lockfiles = remove_stale_nested_lockfiles(REPO_ROOT, args.dry_run)
        lockfile_packages = update_lockfile_workspace_versions(
            REPO_ROOT, version, args.dry_run
        )
        if args.dry_run:
            print("Would run: cargo metadata --no-deps --format-version 1")
        else:
            versions = read_workspace_versions(REPO_ROOT)
            verify_workspace_versions(versions, version)

        action = "Would update" if args.dry_run else "Updated"
        print(f"{action} workspace release version to {version} ({tag})")
        if manifest_changed:
            print("- Cargo.toml workspace package version")
        if removed_lockfiles:
            for relative_path in removed_lockfiles:
                print(f"- removed stale nested lockfile: {relative_path}")
        print("- Cargo.lock workspace packages:", ", ".join(sorted(lockfile_packages)))
        print("Next release workflow version/tag:", tag)
        print(f"Package example: bash pipelines/package.sh --version {tag} --platform macos")
        return 0
    except ReleaseVersionError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
