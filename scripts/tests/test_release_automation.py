from __future__ import annotations

import importlib.util
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
registry = load_script("check_registry_availability")
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
            "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
            "actions/checkout@v7",
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


if __name__ == "__main__":
    unittest.main()
