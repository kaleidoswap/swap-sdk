from __future__ import annotations

import importlib.util
import hashlib
import io
import json
import os
import tarfile
import tempfile
import unittest
import urllib.error
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]


def load_script(name: str):
    path = ROOT / "scripts" / f"{name}.py"
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


assemble_release = load_script("assemble_release")
published = load_script("download_published_artifacts")
registry = load_script("check_registry_availability")
release_notes = load_script("release_notes")
release_ref = load_script("validate_release_ref")
release_version = load_script("release_version")
verify_bundle = load_script("verify_release_bundle")
workflow = load_script("check_release_workflow")


class RegistryAvailabilityTests(unittest.TestCase):
    def test_python_json_api_url_has_json_suffix(self) -> None:
        self.assertEqual(
            registry.version_url(
                "https://test.pypi.org/pypi",
                "kaleidoswap_sdk",
                "0.1.0",
                json_api=True,
            ),
            "https://test.pypi.org/pypi/kaleidoswap_sdk/0.1.0/json",
        )

    def test_404_means_version_is_available(self) -> None:
        error = urllib.error.HTTPError("https://registry.example", 404, "", {}, None)
        self.addCleanup(error.close)
        with mock.patch.object(registry.urllib.request, "urlopen", side_effect=error):
            registry.require_version_available(
                "https://registry.example",
                "@kaleidoswap/sdk",
                "0.1.0",
                "npm",
            )

    def test_existing_version_is_rejected(self) -> None:
        response = mock.MagicMock()
        response.__enter__.return_value = io.StringIO('{"version":"0.1.0"}')
        with mock.patch.object(
            registry.urllib.request, "urlopen", return_value=response
        ):
            with self.assertRaisesRegex(ValueError, "already exists"):
                registry.require_version_available(
                    "https://registry.example",
                    "@kaleidoswap/sdk",
                    "0.1.0",
                    "npm",
                )

    def test_public_pypi_must_be_explicitly_disabled(self) -> None:
        with mock.patch.dict(
            os.environ,
            {
                "NPM_PUBLISH_ENABLED": "false",
                "PYPI_PUBLISH_ENABLED": "true",
                "TEST_PYPI_PUBLISH_ENABLED": "false",
            },
            clear=True,
        ):
            with self.assertRaisesRegex(ValueError, "public PyPI"):
                registry.validate_configuration()

    def test_oidc_registry_flags_accept_enabled_publishers(self) -> None:
        with mock.patch.dict(
            os.environ,
            {
                "NPM_PUBLISH_ENABLED": "true",
                "PYPI_PUBLISH_ENABLED": "false",
                "TEST_PYPI_PUBLISH_ENABLED": "true",
            },
            clear=True,
        ):
            self.assertEqual(registry.validate_configuration(), (True, True))

    def test_registry_flags_reject_implicit_values(self) -> None:
        with mock.patch.dict(
            os.environ,
            {
                "NPM_PUBLISH_ENABLED": "1",
                "PYPI_PUBLISH_ENABLED": "false",
                "TEST_PYPI_PUBLISH_ENABLED": "false",
            },
            clear=True,
        ):
            with self.assertRaisesRegex(ValueError, "true or false"):
                registry.validate_configuration()

    def test_rehearsal_checks_testpypi_while_publisher_is_disabled(self) -> None:
        with (
            mock.patch.dict(
                os.environ,
                {
                    "NPM_PUBLISH_ENABLED": "false",
                    "PYPI_PUBLISH_ENABLED": "false",
                    "TEST_PYPI_PUBLISH_ENABLED": "false",
                },
                clear=True,
            ),
            mock.patch.object(
                registry, "require_version_available"
            ) as require_available,
            mock.patch(
                "sys.argv",
                [
                    "check_registry_availability.py",
                    "0.1.0",
                    "--check-test-pypi",
                ],
            ),
        ):
            self.assertEqual(registry.main(), 0)
        self.assertEqual(require_available.call_count, 2)


class ReleaseNotesTests(unittest.TestCase):
    def test_finalized_release_notes_are_extracted(self) -> None:
        contents = """# Changelog

## [Unreleased]

## [0.1.0] - 2026-07-28

### Added

- Release automation.

## [0.0.1]

- Previous release.
"""
        self.assertEqual(
            release_notes.extract_release_notes(contents, "0.1.0"),
            "### Added\n\n- Release automation.",
        )

    def test_missing_release_notes_are_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "no finalized"):
            release_notes.extract_release_notes(
                "# Changelog\n\n## [Unreleased]\n", "0.1.0"
            )

    def test_empty_release_notes_are_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "empty"):
            release_notes.extract_release_notes(
                "# Changelog\n\n## [0.1.0]\n\n## [0.0.1]\n\nPrevious.",
                "0.1.0",
            )


class PublishedArtifactTests(unittest.TestCase):
    @staticmethod
    def entry(contents: bytes) -> dict:
        return {
            "sha256": hashlib.sha256(contents).hexdigest(),
            "size": len(contents),
        }

    def test_npm_download_must_match_sealed_bundle(self) -> None:
        contents = b"exact npm tarball"
        entries = {"kaleidoswap-sdk-0.1.0.tgz": self.entry(contents)}
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp)
            with (
                mock.patch.object(
                    published,
                    "request_json",
                    return_value={
                        "name": "@kaleidoswap/sdk",
                        "version": "0.1.0",
                        "dist": {"tarball": "https://registry.example/sdk.tgz"},
                    },
                ),
                mock.patch.object(
                    published,
                    "download",
                    side_effect=lambda _url, destination: destination.write_bytes(
                        contents
                    ),
                ),
            ):
                path = published.download_npm(
                    entries,
                    output,
                    "0.1.0",
                    registry="https://registry.example",
                    attempts=1,
                    delay=0,
                )
        self.assertEqual(path.name, "kaleidoswap-sdk-0.1.0.tgz")

    def test_npm_download_rejects_changed_bytes(self) -> None:
        entries = {"kaleidoswap-sdk-0.1.0.tgz": self.entry(b"expected")}
        with tempfile.TemporaryDirectory() as temp:
            with (
                mock.patch.object(
                    published,
                    "request_json",
                    return_value={
                        "name": "@kaleidoswap/sdk",
                        "version": "0.1.0",
                        "dist": {"tarball": "https://registry.example/sdk.tgz"},
                    },
                ),
                mock.patch.object(
                    published,
                    "download",
                    side_effect=lambda _url, destination: destination.write_bytes(
                        b"changed"
                    ),
                ),
                self.assertRaisesRegex(ValueError, "size mismatch"),
            ):
                published.download_npm(
                    entries,
                    Path(temp),
                    "0.1.0",
                    registry="https://registry.example",
                    attempts=1,
                    delay=0,
                )

    def test_testpypi_inventory_and_downloads_match_sealed_bundle(self) -> None:
        contents = b"exact Python artifact"
        names = (
            "kaleidoswap_sdk-0.1.0-py3-none-manylinux_2_28_x86_64.whl",
            "kaleidoswap_sdk-0.1.0-py3-none-manylinux_2_28_aarch64.whl",
            "kaleidoswap_sdk-0.1.0-py3-none-macosx_10_12_x86_64.whl",
            "kaleidoswap_sdk-0.1.0-py3-none-macosx_11_0_arm64.whl",
            "kaleidoswap_sdk-0.1.0-py3-none-win_amd64.whl",
            "kaleidoswap_sdk-0.1.0.tar.gz",
        )
        entries = {name: self.entry(contents) for name in names}
        payload = {
            "info": {"version": "0.1.0"},
            "urls": [
                {
                    "filename": name,
                    "digests": {"sha256": entries[name]["sha256"]},
                    "url": f"https://registry.example/{name}",
                }
                for name in names
            ],
        }
        with tempfile.TemporaryDirectory() as temp:
            with (
                mock.patch.object(published, "request_json", return_value=payload),
                mock.patch.object(
                    published,
                    "download",
                    side_effect=lambda _url, destination: destination.write_bytes(
                        contents
                    ),
                ),
            ):
                wheel, sdist = published.download_test_pypi(
                    entries,
                    Path(temp),
                    "0.1.0",
                    registry="https://registry.example",
                    attempts=1,
                    delay=0,
                )
        self.assertTrue(wheel.name.endswith("manylinux_2_28_x86_64.whl"))
        self.assertTrue(sdist.name.endswith(".tar.gz"))

    def test_testpypi_missing_artifact_is_rejected(self) -> None:
        contents = b"artifact"
        wheel = "kaleidoswap_sdk-0.1.0-py3-none-manylinux_2_28_x86_64.whl"
        sdist = "kaleidoswap_sdk-0.1.0.tar.gz"
        entries = {wheel: self.entry(contents), sdist: self.entry(contents)}
        payload = {
            "info": {"version": "0.1.0"},
            "urls": [
                {
                    "filename": sdist,
                    "digests": {"sha256": entries[sdist]["sha256"]},
                    "url": "https://registry.example/sdist",
                }
            ],
        }
        with (
            mock.patch.object(published, "request_json", return_value=payload),
            self.assertRaisesRegex(ValueError, "inventory"),
        ):
            published.download_test_pypi(
                entries,
                Path("/unused"),
                "0.1.0",
                registry="https://registry.example",
                attempts=1,
                delay=0,
            )


class ReleaseArtifactTests(unittest.TestCase):
    def make_npm_tarball(self, directory: Path, version: str) -> Path:
        path = directory / f"kaleidoswap-sdk-{version}.tgz"
        package = {"name": "@kaleidoswap/sdk", "version": version}
        members = {
            name: b"placeholder"
            for name in assemble_release.NPM_REQUIRED
            if name != "package/package.json"
        }
        members["package/package.json"] = json.dumps(package).encode()
        with tarfile.open(path, "w:gz") as archive:
            for name, contents in members.items():
                info = tarfile.TarInfo(name)
                info.size = len(contents)
                archive.addfile(info, io.BytesIO(contents))
        return path

    def make_inventory(self, directory: Path, version: str) -> None:
        wheel_tags = (
            "cp311-cp311-manylinux_2_28_x86_64.whl",
            "cp311-cp311-manylinux_2_28_aarch64.whl",
            "cp311-cp311-macosx_11_0_x86_64.whl",
            "cp311-cp311-macosx_11_0_arm64.whl",
            "cp311-cp311-win_amd64.whl",
        )
        for tag in wheel_tags:
            (directory / f"kaleidoswap_sdk-{version}-{tag}").write_bytes(b"wheel")
        (directory / f"kaleidoswap_sdk-{version}.tar.gz").write_bytes(b"sdist")
        self.make_npm_tarball(directory, version)

    def test_exact_cross_platform_inventory_is_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            self.make_inventory(directory, "0.1.0")
            artifacts = assemble_release.collect_artifacts(directory, "0.1.0")
            self.assertEqual(len(artifacts), 7)

    def test_missing_platform_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            self.make_inventory(directory, "0.1.0")
            next(directory.glob("*win_amd64.whl")).unlink()
            with self.assertRaisesRegex(ValueError, "five wheels"):
                assemble_release.collect_artifacts(directory, "0.1.0")

    def test_invalid_npm_archive_is_rejected_before_release_ready(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            self.make_inventory(directory, "0.1.0")
            npm = next(directory.glob("*.tgz"))
            with tarfile.open(npm, "w:gz") as archive:
                package = b'{"name":"@kaleidoswap/sdk","version":"0.1.0"}'
                info = tarfile.TarInfo("package/package.json")
                info.size = len(package)
                archive.addfile(info, io.BytesIO(package))
            with self.assertRaisesRegex(ValueError, "missing required files"):
                assemble_release.collect_artifacts(directory, "0.1.0")

    def make_release_bundle(self, directory: Path) -> None:
        self.make_inventory(directory, "0.1.0")
        artifacts = assemble_release.collect_artifacts(directory, "0.1.0")
        assemble_release.write_release_metadata(
            directory,
            artifacts,
            "0.1.0",
            "v0.1.0",
            "HEAD",
        )

    def test_exact_release_bundle_is_verified(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            self.make_release_bundle(directory)
            verify_bundle.verify(
                directory,
                version="0.1.0",
                tag="v0.1.0",
                commit="HEAD",
            )

    def test_tampered_release_bundle_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            self.make_release_bundle(directory)
            next(directory.glob("*win_amd64.whl")).write_bytes(b"tampered")
            with self.assertRaisesRegex(ValueError, "checksum mismatch"):
                verify_bundle.verify(
                    directory,
                    version="0.1.0",
                    tag="v0.1.0",
                    commit="HEAD",
                )

    def test_unexpected_release_asset_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            self.make_release_bundle(directory)
            (directory / "unexpected.txt").write_text("unexpected")
            with self.assertRaisesRegex(ValueError, "expected 10 release assets"):
                verify_bundle.verify(
                    directory,
                    version="0.1.0",
                    tag="v0.1.0",
                    commit="HEAD",
                )


class ReleaseRefTests(unittest.TestCase):
    def test_malformed_rehearsal_tag_is_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "format vX.Y.Z"):
            release_version.validate_tag("release-0.1.0")

    def test_rehearsal_version_mismatch_is_rejected(self) -> None:
        with (
            mock.patch.object(
                release_version,
                "versions",
                return_value={
                    "Rust package": "0.1.0",
                    "Rust lockfile": "0.1.0",
                    "Python package": "0.1.0",
                    "Python lockfile": "0.1.0",
                    "TypeScript package": "0.1.0",
                    "TypeScript lockfile": "0.1.0",
                },
            ),
            self.assertRaisesRegex(ValueError, "does not match"),
        ):
            release_version.validate_tag("v0.1.1")

    def test_rehearsal_source_must_be_based_on_trunk(self) -> None:
        with (
            mock.patch.object(release_ref, "validate_tag"),
            mock.patch.object(
                release_ref,
                "git",
                side_effect=["source-commit", "trunk-commit"],
            ),
            mock.patch.object(release_ref, "require_ancestor") as require_ancestor,
        ):
            release_ref.validate_rehearsal_ref(
                "v0.1.0",
                "source-sha",
                "origin/trunk",
            )
        require_ancestor.assert_called_once_with(
            "trunk-commit",
            "source-commit",
            "source-commit is not based on origin/trunk",
        )


class WorkflowInvariantTests(unittest.TestCase):
    def test_committed_workflow_passes(self) -> None:
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        workflow.validate(contents)

    def test_skip_existing_is_rejected(self) -> None:
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        with self.assertRaisesRegex(ValueError, "skip"):
            workflow.validate(contents + "\n# --skip-existing\n")

    def test_mutable_action_is_rejected(self) -> None:
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        changed = contents.replace(
            "actions/setup-node@820762786026740c76f36085b0efc47a31fe5020",
            "actions/setup-node@v7",
            1,
        )
        with self.assertRaisesRegex(ValueError, "mutable"):
            workflow.validate(changed)

    def test_registry_secret_is_rejected(self) -> None:
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        with self.assertRaisesRegex(ValueError, "stored registry credentials"):
            workflow.validate(contents + "\n# secrets.NPM_TOKEN\n")

    def test_extra_oidc_permission_is_rejected(self) -> None:
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        with self.assertRaisesRegex(ValueError, "exactly two"):
            workflow.validate(contents + "\n# id-token: write\n")

    def test_production_release_requires_npm_activation(self) -> None:
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        changed = contents.replace(
            'test "${NPM_PUBLISH_ENABLED}" = "true"',
            'test "${NPM_PUBLISH_ENABLED}" = "false"',
            1,
        )
        with self.assertRaisesRegex(ValueError, "activation"):
            workflow.validate(changed)

    def test_registry_download_verification_is_required(self) -> None:
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        changed = contents.replace(
            "scripts/download_published_artifacts.py",
            "scripts/omitted_registry_verifier.py",
        )
        with self.assertRaisesRegex(ValueError, "missing invariants"):
            workflow.validate(changed)

    def test_production_github_release_cannot_remain_draft(self) -> None:
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        with self.assertRaisesRegex(ValueError, "draft"):
            workflow.validate(contents + "\n  # --draft\n")

    def test_release_metadata_uses_peeled_source_commit(self) -> None:
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        build = (ROOT / ".github/workflows/release-build.yaml").read_text()
        changed = build.replace(
            '--commit "${{ needs.preflight.outputs.commit }}"',
            '--commit "${GITHUB_SHA}"',
        )
        with self.assertRaisesRegex(ValueError, "peeled"):
            workflow.validate(contents, build_contents=changed)

    def test_rehearsal_caller_cannot_request_oidc(self) -> None:
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        rehearsal = (
            ROOT / ".github/workflows/release-rehearsal.yaml"
        ).read_text() + "\n# id-token: write\n"
        with self.assertRaisesRegex(ValueError, "release authority"):
            workflow.validate(contents, rehearsal)

    def test_read_only_build_cannot_request_oidc(self) -> None:
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        build = (
            ROOT / ".github/workflows/release-build.yaml"
        ).read_text() + "\n# id-token: write\n"
        with self.assertRaisesRegex(ValueError, "release authority"):
            workflow.validate(contents, build_contents=build)

    def test_rehearsal_cannot_hardcode_a_version(self) -> None:
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        rehearsal = (
            (ROOT / ".github/workflows/release-rehearsal.yaml")
            .read_text()
            .replace('release_tag: ""', "release_tag: v0.1.0")
        )
        with self.assertRaisesRegex(ValueError, "must not hardcode a version"):
            workflow.validate(contents, rehearsal)

    def test_rehearsal_must_not_require_an_unclaimed_version(self) -> None:
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        build = (
            (ROOT / ".github/workflows/release-build.yaml")
            .read_text()
            .replace("--flags-only", "--check-test-pypi")
        )
        with self.assertRaisesRegex(ValueError, "still be unclaimed"):
            workflow.validate(contents, build_contents=build)

    def test_publisher_must_reverify_sealed_bytes(self) -> None:
        contents = (
            (ROOT / ".github/workflows/release.yaml")
            .read_text()
            .replace(
                "sha256sum --check --strict SHA256SUMS",
                "true # skipped",
                1,
            )
        )
        with self.assertRaisesRegex(ValueError, "re-verify the sealed bundle"):
            workflow.validate(contents)

    def test_github_release_must_verify_the_sealed_bundle(self) -> None:
        contents = (
            (ROOT / ".github/workflows/release.yaml")
            .read_text()
            .replace(
                "scripts/verify_release_bundle.py",
                "scripts/omitted_bundle_verifier.py",
            )
        )
        with self.assertRaisesRegex(ValueError, "verify the sealed bundle"):
            workflow.validate(contents)

    def test_release_artifact_names_are_stable_across_attempts(self) -> None:
        """Re-running failed jobs must find the bundle the first attempt sealed."""
        build = (ROOT / ".github/workflows/release-build.yaml").read_text()
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        self.assertNotIn("github.run_attempt", build)
        self.assertNotIn("github.run_attempt", contents)


class RuntimeVersionTests(unittest.TestCase):
    def test_python_package_reports_a_version_without_a_second_source(self) -> None:
        source = (ROOT / "bindings/python/kaleidoswap_sdk/__init__.py").read_text()
        self.assertIn("__version__", source)
        # Derived from installed metadata, so it can never drift from pyproject.
        self.assertIn("_distribution_version", source)
        self.assertNotIn('__version__ = "0.1.0"', source)


if __name__ == "__main__":
    unittest.main()
