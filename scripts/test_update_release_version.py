#!/usr/bin/env python3
"""Tests for the release-version updater."""

from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path


SCRIPT_PATH = Path(__file__).with_name("update_release_version.py")
SPEC = importlib.util.spec_from_file_location("update_release_version", SCRIPT_PATH)
update_release_version = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(update_release_version)


class ReleaseVersionTests(unittest.TestCase):
    def test_parse_release_version_accepts_bare_semver(self) -> None:
        self.assertEqual(update_release_version.parse_release_version("1.2.3"), "1.2.3")
        self.assertEqual(update_release_version.release_tag("1.2.3"), "v1.2.3")

    def test_parse_release_version_rejects_tags_and_partial_versions(self) -> None:
        invalid_versions = ["v1.2.3", "1.2", "1.2.3-beta.1", "01.2.3", "1.02.3"]
        for version in invalid_versions:
            with self.subTest(version=version):
                with self.assertRaises(update_release_version.ReleaseVersionError):
                    update_release_version.parse_release_version(version)

    def test_replace_workspace_version_text_updates_only_workspace_package(self) -> None:
        contents = """[workspace]
members = ["core"]

[workspace.package]
version = "1.2.0"

[profile.release]
opt-level = 3
"""
        updated = update_release_version.replace_workspace_version_text(contents, "1.2.1")

        self.assertIn('version = "1.2.1"', updated)
        self.assertNotIn('version = "1.2.0"', updated)
        self.assertIn("[profile.release]", updated)

    def test_replace_workspace_version_text_requires_workspace_package(self) -> None:
        with self.assertRaises(update_release_version.ReleaseVersionError):
            update_release_version.replace_workspace_version_text("[workspace]\n", "1.2.1")

    def test_update_workspace_manifest_dry_run_does_not_write(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            cargo_toml = root / "Cargo.toml"
            cargo_toml.write_text(
                '[workspace]\nmembers = []\n\n[workspace.package]\nversion = "1.2.0"\n',
                encoding="utf-8",
            )

            changed = update_release_version.update_workspace_manifest(root, "1.2.1", dry_run=True)

            self.assertTrue(changed)
            self.assertIn('version = "1.2.0"', cargo_toml.read_text(encoding="utf-8"))

    def test_replace_lockfile_workspace_versions_updates_only_local_packages(self) -> None:
        contents = """[[package]]
name = "client"
version = "1.2.0"
dependencies = []

[[package]]
name = "client-utils"
version = "0.1.0"

[[package]]
name = "server-utils"
version = "1.1.0"

[[package]]
name = "server-utils"
version = "0.0.1"
source = "registry+https://github.com/rust-lang/crates.io-index"
"""

        updated, package_names = update_release_version.replace_lockfile_workspace_versions_text(
            contents, "1.2.1", {"client", "client-utils", "server-utils"}
        )

        self.assertEqual(package_names, {"client", "client-utils", "server-utils"})
        self.assertIn('name = "client"\nversion = "1.2.1"', updated)
        self.assertIn('name = "client-utils"\nversion = "1.2.1"', updated)
        self.assertIn('name = "server-utils"\nversion = "1.2.1"', updated)
        self.assertIn('name = "server-utils"\nversion = "0.0.1"\nsource =', updated)

    def test_remove_stale_nested_lockfiles_supports_dry_run(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            lockfile = root / "client" / "utils" / "Cargo.lock"
            lockfile.parent.mkdir(parents=True)
            lockfile.write_text("generated", encoding="utf-8")

            removed = update_release_version.remove_stale_nested_lockfiles(root, dry_run=True)

            self.assertEqual(removed, [Path("client/utils/Cargo.lock")])
            self.assertTrue(lockfile.exists())

    def test_workspace_versions_from_metadata_filters_workspace_members(self) -> None:
        metadata = {
            "workspace_members": ["api-id", "client-id"],
            "packages": [
                {"id": "api-id", "name": "api", "version": "1.2.1"},
                {"id": "client-id", "name": "client", "version": "1.2.1"},
                {"id": "dependency-id", "name": "serde", "version": "1.0.0"},
            ],
        }

        versions = update_release_version.workspace_versions_from_metadata(metadata)

        self.assertEqual(versions, {"api": "1.2.1", "client": "1.2.1"})

    def test_verify_workspace_versions_reports_mismatches(self) -> None:
        versions = {name: "1.2.1" for name in update_release_version.EXPECTED_WORKSPACE_PACKAGES}
        versions["server-utils"] = "1.1.0"

        with self.assertRaises(update_release_version.ReleaseVersionError):
            update_release_version.verify_workspace_versions(versions, "1.2.1")


if __name__ == "__main__":
    unittest.main()
