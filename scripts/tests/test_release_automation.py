from __future__ import annotations

import importlib.util
import hashlib
import io
import json
import os
import re
import sys
import tarfile
import tempfile
import unittest
import urllib.error
import zipfile
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
# Scripts import their shared release_metadata module the way they do when run
# directly (sys.path[0] == scripts/); spec_from_file_location does not set that.
if str(ROOT / "scripts") not in sys.path:
    sys.path.insert(0, str(ROOT / "scripts"))


def load_script(name: str):
    path = ROOT / "scripts" / f"{name}.py"
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


assemble_release = load_script("assemble_release")
published = load_script("download_published_artifacts")
react_native = load_script("react_native_release")
registry = load_script("check_registry_availability")
release_notes = load_script("release_notes")
release_ref = load_script("validate_release_ref")
release_version = load_script("release_version")
verify_bundle = load_script("verify_release_bundle")
workflow = load_script("check_release_workflow")

import release_metadata  # noqa: E402  (needs the sys.path set up above)

NPM_PACKAGE = release_metadata.npm_package()
NPM_TARBALL_0_1_0 = release_metadata.npm_tarball_name("0.1.0")
RN_NPM_PACKAGE = release_metadata.react_native_npm_package()
RN_NPM_TARBALL_0_1_0 = release_metadata.react_native_npm_tarball_name("0.1.0")


class RegistryAvailabilityTests(unittest.TestCase):
    def test_python_json_api_url_has_json_suffix(self) -> None:
        self.assertEqual(
            registry.version_url(
                "https://pypi.org/pypi",
                "kaleidorg_swap_sdk",
                "0.1.0",
                json_api=True,
            ),
            "https://pypi.org/pypi/kaleidorg_swap_sdk/0.1.0/json",
        )

    def test_404_means_version_is_available(self) -> None:
        error = urllib.error.HTTPError("https://registry.example", 404, "", {}, None)
        self.addCleanup(error.close)
        with mock.patch.object(registry.urllib.request, "urlopen", side_effect=error):
            registry.require_version_available(
                "https://registry.example",
                NPM_PACKAGE,
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
                    NPM_PACKAGE,
                    "0.1.0",
                    "npm",
                )

    def test_public_pypi_may_now_be_enabled(self) -> None:
        # The distribution rename cleared the name collision that made public
        # PyPI impossible, so an enabled flag is accepted rather than rejected.
        with mock.patch.dict(
            os.environ,
            {
                "NPM_PUBLISH_ENABLED": "false",
                "PYPI_PUBLISH_ENABLED": "true",
            },
            clear=True,
        ):
            self.assertEqual(registry.validate_configuration(), (False, True))

    def test_registry_flags_accept_enabled_publishers(self) -> None:
        with mock.patch.dict(
            os.environ,
            {
                "NPM_PUBLISH_ENABLED": "true",
                "PYPI_PUBLISH_ENABLED": "true",
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
            },
            clear=True,
        ):
            with self.assertRaisesRegex(ValueError, "true or false"):
                registry.validate_configuration()

    def test_rehearsal_checks_pypi_while_publisher_is_disabled(self) -> None:
        with (
            mock.patch.dict(
                os.environ,
                {
                    "NPM_PUBLISH_ENABLED": "false",
                    "PYPI_PUBLISH_ENABLED": "false",
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
                    "--check-pypi",
                ],
            ),
        ):
            self.assertEqual(registry.main(), 0)
        # Both npm packages, then PyPI.
        self.assertEqual(require_available.call_count, 3)


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


class RegistryPropagationTests(unittest.TestCase):
    """A published version that has not propagated must cost a wait, not a release.

    The 0.8.0 run published both npm packages cleanly and then failed its
    verification 113s later, still reading 404. That failed the registry gate,
    which skipped the GitHub release -- and the React Native package's
    postinstall fetches its native archives from exactly that release, so a
    registry delay of a couple of minutes broke every install of the published
    package until the job was re-run by hand.
    """

    URL = "https://registry.example/%40scope%2Fpkg/0.1.0"

    @staticmethod
    def http_error(code: int) -> urllib.error.HTTPError:
        return urllib.error.HTTPError(
            RegistryPropagationTests.URL, code, "nope", {}, None
        )

    def responses(self, *results):
        """Drive request_json through a scripted sequence of urlopen outcomes."""
        remaining = list(results)

        def urlopen(_request, timeout=None):  # noqa: ARG001
            result = remaining.pop(0)
            if isinstance(result, Exception):
                raise result
            return io.BytesIO(json.dumps(result).encode())

        return urlopen

    def test_version_that_propagates_late_is_still_verified(self) -> None:
        payload = {"name": "@scope/pkg", "version": "0.1.0"}
        urlopen = self.responses(
            self.http_error(404),
            self.http_error(404),
            self.http_error(404),
            payload,
        )
        with (
            mock.patch.object(published.urllib.request, "urlopen", urlopen),
            mock.patch.object(published.time, "sleep") as sleep,
        ):
            self.assertEqual(
                published.request_json(self.URL, 11, 10.0, max_delay=60.0),
                payload,
            )
        self.assertEqual([call.args[0] for call in sleep.call_args_list], [10, 20, 40])

    def test_exhausted_budget_names_the_last_status_and_the_wait(self) -> None:
        urlopen = self.responses(*[self.http_error(404)] * 3)
        with (
            mock.patch.object(published.urllib.request, "urlopen", urlopen),
            mock.patch.object(published.time, "sleep"),
            self.assertRaisesRegex(ValueError, r"over 30s of backoff, last HTTP 404"),
        ):
            published.request_json(self.URL, 3, 10.0, max_delay=20.0)

    def test_status_that_will_not_fix_itself_fails_at_once(self) -> None:
        urlopen = self.responses(self.http_error(403))
        with (
            mock.patch.object(published.urllib.request, "urlopen", urlopen),
            mock.patch.object(published.time, "sleep") as sleep,
            self.assertRaisesRegex(ValueError, "registry returned HTTP 403"),
        ):
            published.request_json(self.URL, 11, 10.0, max_delay=60.0)
        sleep.assert_not_called()

    def test_registry_fault_and_throttling_are_waited_out(self) -> None:
        payload = {"name": "@scope/pkg", "version": "0.1.0"}
        urlopen = self.responses(
            self.http_error(503),
            self.http_error(429),
            OSError("connection reset"),
            payload,
        )
        with (
            mock.patch.object(published.urllib.request, "urlopen", urlopen),
            mock.patch.object(published.time, "sleep"),
        ):
            self.assertEqual(
                published.request_json(self.URL, 5, 1.0, max_delay=1.0), payload
            )

    def test_backoff_doubles_up_to_its_cap(self) -> None:
        self.assertEqual(
            published.backoff(10.0, 60.0, 8),
            [10.0, 20.0, 40.0, 60.0, 60.0, 60.0, 60.0],
        )
        # A single attempt never sleeps: there is no retry to pause before.
        self.assertEqual(published.backoff(10.0, 60.0, 1), [])

    def run_main(self, *flags: str) -> tuple[int, dict]:
        """Drive the CLI over a minimal bundle, capturing the resolved budget."""
        captured: dict = {}

        def download_npm(*_args, **kwargs):
            captured.update(kwargs)
            return Path("unused")

        with tempfile.TemporaryDirectory() as temp:
            bundle = Path(temp) / "bundle"
            bundle.mkdir()
            (bundle / "release-manifest.json").write_text(
                json.dumps({"version": "0.1.0", "artifacts": []}), encoding="utf-8"
            )
            argv = [
                "download_published_artifacts.py",
                str(bundle),
                str(Path(temp) / "out"),
                "--version",
                "0.1.0",
                "--npm",
                *flags,
            ]
            with (
                mock.patch.object(sys, "argv", argv),
                mock.patch.object(published, "download_npm", download_npm),
            ):
                return published.main(), captured

    def test_raising_the_first_delay_alone_is_accepted(self) -> None:
        """--delay stood on its own before the cap existed; it still must.

        The default cap is not a floor the caller agreed to, so validating a
        supplied --delay against it turned a working invocation into an error.
        """
        status, captured = self.run_main("--delay", "120")
        self.assertEqual(status, 0)
        self.assertEqual(captured["delay"], 120)
        self.assertGreaterEqual(captured["max_delay"], captured["delay"])

    def test_an_explicitly_inverted_pair_is_still_rejected(self) -> None:
        status, _ = self.run_main("--delay", "120", "--max-delay", "60")
        self.assertEqual(status, 1)

    def test_npm_budget_covers_the_delay_that_broke_0_8_0(self) -> None:
        """113s of patience is what failed. Keep the budget far past it."""
        budget = sum(
            published.backoff(
                published.NPM_DELAY, published.NPM_MAX_DELAY, published.NPM_ATTEMPTS
            )
        )
        self.assertGreaterEqual(budget, 300)

    @staticmethod
    def budget(attempts: int, delay: float, max_delay: float, timeout: int) -> float:
        """One read's worst case: every backoff plus every attempt hanging.

        Sleeping is not the whole cost. A registry that hangs rather than 404s
        also burns the read timeout on every attempt -- which is exactly the
        case the job timeout is a backstop for.
        """
        return sum(published.backoff(delay, max_delay, attempts)) + attempts * timeout

    @classmethod
    def download_worst_case(cls) -> float:
        """The longest one artifact download may take before it gives up."""
        return cls.budget(
            published.DOWNLOAD_ATTEMPTS,
            published.DOWNLOAD_DELAY,
            published.DOWNLOAD_MAX_DELAY,
            published.DOWNLOAD_TIMEOUT,
        )

    @classmethod
    def npm_worst_case(cls) -> float:
        """The longest verify-npm can spend waiting on the registry.

        Per npm package the job reads metadata once and then downloads the
        tarball, and both waits are bounded and budgeted -- the download used
        to be a single unguarded fetch, so it cost nothing here and could fail
        the whole release on one bad response.
        """
        metadata = cls.budget(
            published.NPM_ATTEMPTS,
            published.NPM_DELAY,
            published.NPM_MAX_DELAY,
            published.REQUEST_TIMEOUT,
        )
        return (metadata + cls.download_worst_case()) * (
            release_metadata.NPM_PACKAGE_COUNT
        )

    @classmethod
    def pypi_worst_case(cls) -> float:
        """The same for verify-pypi: one metadata read, then every artifact."""
        metadata = cls.budget(
            published.PYPI_ATTEMPTS,
            published.PYPI_DELAY,
            published.PYPI_MAX_DELAY,
            published.REQUEST_TIMEOUT,
        )
        downloads = cls.download_worst_case() * published.PYTHON_ARTIFACT_COUNT
        return metadata + downloads

    @staticmethod
    def job_timeout(name: str) -> int:
        job = workflow.production_jobs(
            (ROOT / ".github/workflows/release.yaml").read_text()
        )[name]
        timeout = re.search(r"timeout-minutes:\s*(\d+)", job)
        assert timeout is not None
        return int(timeout.group(1)) * 60

    def test_npm_verifier_outlives_its_own_propagation_budget(self) -> None:
        """The job timeout and the script budget must not drift apart again.

        Headroom on top of the worst case is for the job's own work: checkout,
        Node, the bundle download, and three clean-install smoke tests, one of
        which fetches Firefox. Every wait the script may spend is counted here,
        so adding an unbudgeted one underneath reopens the 0.8.0 gap.
        """
        self.assertGreater(
            self.job_timeout("verify-npm"), self.npm_worst_case() + 5 * 60
        )

    def test_pypi_verifier_outlives_its_own_download_budget(self) -> None:
        """Six artifacts, each with a retry budget, under one job timeout."""
        self.assertGreater(
            self.job_timeout("verify-pypi"), self.pypi_worst_case() + 5 * 60
        )


class ArtifactDownloadTests(unittest.TestCase):
    """The body download must survive what the metadata read already survives.

    The propagation budget stops at the metadata read; the fetch immediately
    after it used to be a single unguarded attempt. A CDN that 503s or drops
    the connection mid-body fails the registry gate and skips the GitHub
    release -- the 0.8.0 failure one step later, with the React Native package
    published and its native archives nowhere to fetch.
    """

    URL = "https://cdn.example/kaleidorg-swap-sdk-0.1.0.tgz"
    BODY = b"exact published bytes"

    class Response(io.BytesIO):
        """A body that reports a Content-Length, truthfully or not."""

        def __init__(self, body: bytes, declared: int | None = None) -> None:
            super().__init__(body)
            length = len(body) if declared is None else declared
            self.headers = {"Content-Length": str(length)}

    @classmethod
    def http_error(cls, code: int) -> urllib.error.HTTPError:
        return urllib.error.HTTPError(cls.URL, code, "nope", {}, None)

    def responses(self, *results):
        remaining = list(results)

        def urlopen(_request, timeout=None):  # noqa: ARG001
            result = remaining.pop(0)
            if isinstance(result, Exception):
                raise result
            return result

        return urlopen

    def run_download(self, *results, attempts: int = 3):
        """Download into a fresh directory; return the destination and sleeps."""
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        destination = Path(temp.name) / "kaleidorg-swap-sdk-0.1.0.tgz"
        with (
            mock.patch.object(
                published.urllib.request, "urlopen", self.responses(*results)
            ),
            mock.patch.object(published.time, "sleep") as sleep,
        ):
            try:
                published.download(
                    self.URL,
                    destination,
                    attempts=attempts,
                    delay=1.0,
                    max_delay=1.0,
                )
            finally:
                self.leftovers = sorted(
                    path.name for path in Path(temp.name).iterdir()
                )
        return destination, sleep

    def test_transient_registry_answer_is_waited_out(self) -> None:
        destination, sleep = self.run_download(
            self.http_error(503),
            self.http_error(404),
            self.Response(self.BODY),
        )
        self.assertEqual(destination.read_bytes(), self.BODY)
        self.assertEqual(sleep.call_count, 2)
        self.assertEqual(self.leftovers, [destination.name])

    def test_status_that_will_not_fix_itself_fails_at_once(self) -> None:
        with self.assertRaisesRegex(ValueError, "registry returned HTTP 403"):
            _, sleep = self.run_download(self.http_error(403))
        # Nothing on disk to mistake for a download, not even a partial file.
        self.assertEqual(self.leftovers, [])

    def test_truncated_body_is_retried_rather_than_kept(self) -> None:
        """A dropped connection must not read as a checksum mismatch.

        The caller hashes whatever is on disk, so a short body kept under the
        final name surfaces as tampering rather than as the network fault it
        is.
        """
        destination, _ = self.run_download(
            self.Response(self.BODY[:5], len(self.BODY)),
            self.Response(self.BODY),
        )
        self.assertEqual(destination.read_bytes(), self.BODY)
        self.assertEqual(self.leftovers, [destination.name])

    def test_exhausted_budget_names_the_status_and_leaves_nothing(self) -> None:
        with self.assertRaisesRegex(
            ValueError,
            r"published artifact was unavailable after 3 attempts.*last HTTP 503",
        ):
            self.run_download(*[self.http_error(503)] * 3)
        self.assertEqual(self.leftovers, [])

    def test_a_body_that_never_completes_is_not_left_behind(self) -> None:
        with self.assertRaisesRegex(ValueError, "body ended after 5 of 21 bytes"):
            self.run_download(
                *(self.Response(self.BODY[:5], len(self.BODY)) for _ in range(3)),
            )
        self.assertEqual(self.leftovers, [])


class PublishedArtifactTests(unittest.TestCase):
    @staticmethod
    def entry(contents: bytes) -> dict:
        return {
            "sha256": hashlib.sha256(contents).hexdigest(),
            "size": len(contents),
        }

    def test_npm_download_must_match_sealed_bundle(self) -> None:
        contents = b"exact npm tarball"
        entries = {NPM_TARBALL_0_1_0: self.entry(contents)}
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp)
            with (
                mock.patch.object(
                    published,
                    "request_json",
                    return_value={
                        "name": NPM_PACKAGE,
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
        self.assertEqual(path.name, NPM_TARBALL_0_1_0)

    def test_react_native_npm_download_uses_its_own_identity(self) -> None:
        contents = b"exact react native tarball"
        entries = {RN_NPM_TARBALL_0_1_0: self.entry(contents)}
        with tempfile.TemporaryDirectory() as temp:
            with (
                mock.patch.object(
                    published,
                    "request_json",
                    return_value={
                        "name": RN_NPM_PACKAGE,
                        "version": "0.1.0",
                        "dist": {"tarball": "https://registry.example/rn.tgz"},
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
                    Path(temp),
                    "0.1.0",
                    registry="https://registry.example",
                    attempts=1,
                    delay=0,
                    package=RN_NPM_PACKAGE,
                )
        self.assertEqual(path.name, RN_NPM_TARBALL_0_1_0)

    def test_npm_download_rejects_changed_bytes(self) -> None:
        entries = {NPM_TARBALL_0_1_0: self.entry(b"expected")}
        with tempfile.TemporaryDirectory() as temp:
            with (
                mock.patch.object(
                    published,
                    "request_json",
                    return_value={
                        "name": NPM_PACKAGE,
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
            "kaleidorg_swap_sdk-0.1.0-py3-none-manylinux_2_28_x86_64.whl",
            "kaleidorg_swap_sdk-0.1.0-py3-none-manylinux_2_28_aarch64.whl",
            "kaleidorg_swap_sdk-0.1.0-py3-none-macosx_10_12_x86_64.whl",
            "kaleidorg_swap_sdk-0.1.0-py3-none-macosx_11_0_arm64.whl",
            "kaleidorg_swap_sdk-0.1.0-py3-none-win_amd64.whl",
            "kaleidorg_swap_sdk-0.1.0.tar.gz",
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
                wheel, sdist = published.download_python_index(
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
        wheel = "kaleidorg_swap_sdk-0.1.0-py3-none-manylinux_2_28_x86_64.whl"
        sdist = "kaleidorg_swap_sdk-0.1.0.tar.gz"
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
            published.download_python_index(
                entries,
                Path("/unused"),
                "0.1.0",
                registry="https://registry.example",
                attempts=1,
                delay=0,
            )


class ReleaseArtifactTests(unittest.TestCase):
    @staticmethod
    def write_tarball(path: Path, members: dict[str, bytes]) -> Path:
        with tarfile.open(path, "w:gz") as archive:
            for name, contents in members.items():
                info = tarfile.TarInfo(name)
                info.size = len(contents)
                archive.addfile(info, io.BytesIO(contents))
        return path

    def make_npm_tarball(self, directory: Path, version: str) -> Path:
        package = {"name": NPM_PACKAGE, "version": version}
        members = {name: b"placeholder" for name in assemble_release.NPM_REQUIRED}
        members["package/package.json"] = json.dumps(package).encode()
        return self.write_tarball(
            directory / release_metadata.npm_tarball_name(version), members
        )

    def make_native_archives(
        self,
        directory: Path,
        version: str,
        *,
        omit: str | None = None,
        contents: bytes = b"compiled",
    ) -> bytes:
        """Write both archives with their full inventories; return the manifest."""
        for name, members in react_native.ARCHIVE_INVENTORY.items():
            with zipfile.ZipFile(directory / name, "w") as archive:
                for member in sorted(members - {omit}):
                    archive.writestr(member, contents)
        manifest = {
            "schema": 1,
            "version": version,
            "artifacts": {
                name: {"sha256": hashlib.sha256((directory / name).read_bytes()).hexdigest()}
                for name in release_metadata.NATIVE_ARCHIVES
            },
        }
        data = (json.dumps(manifest, indent=2) + "\n").encode()
        (directory / release_metadata.NATIVE_MANIFEST).write_bytes(data)
        return data

    def make_react_native_tarball(
        self,
        directory: Path,
        version: str,
        manifest: bytes,
        extra: dict[str, bytes] | None = None,
    ) -> Path:
        package = {
            "name": RN_NPM_PACKAGE,
            "version": version,
            "scripts": {"postinstall": react_native.POSTINSTALL},
        }
        members = {name: b"placeholder" for name in react_native.NPM_REQUIRED}
        members["package/package.json"] = json.dumps(package).encode()
        members[f"package/{release_metadata.NATIVE_MANIFEST}"] = manifest
        members.update(extra or {})
        return self.write_tarball(
            directory / release_metadata.react_native_npm_tarball_name(version),
            members,
        )

    def make_inventory(self, directory: Path, version: str) -> None:
        wheel_tags = (
            "cp311-cp311-manylinux_2_28_x86_64.whl",
            "cp311-cp311-manylinux_2_28_aarch64.whl",
            "cp311-cp311-macosx_11_0_x86_64.whl",
            "cp311-cp311-macosx_11_0_arm64.whl",
            "cp311-cp311-win_amd64.whl",
        )
        for tag in wheel_tags:
            (directory / f"kaleidorg_swap_sdk-{version}-{tag}").write_bytes(b"wheel")
        (directory / f"kaleidorg_swap_sdk-{version}.tar.gz").write_bytes(b"sdist")
        self.make_npm_tarball(directory, version)
        manifest = self.make_native_archives(directory, version)
        self.make_react_native_tarball(directory, version, manifest)

    def test_exact_cross_platform_inventory_is_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            self.make_inventory(directory, "0.1.0")
            artifacts = assemble_release.collect_artifacts(directory, "0.1.0")
            self.assertEqual(len(artifacts), release_metadata.PACKAGE_COUNT)

    def test_react_native_tarball_must_match_the_archives_beside_it(self) -> None:
        # The digests inside the tarball are what postinstall will trust. An
        # archive that does not match them must never reach the release page.
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            self.make_inventory(directory, "0.1.0")
            self.make_native_archives(directory, "0.1.0", contents=b"rebuilt")
            # The loose manifest was rewritten for the new archives; the copy
            # inside the tarball still names the old digests.
            (directory / release_metadata.NATIVE_MANIFEST).write_bytes(
                self.make_native_archives(directory, "0.1.0", contents=b"rebuilt")
            )
            with self.assertRaisesRegex(ValueError, "does not match the digest|differs"):
                assemble_release.collect_artifacts(directory, "0.1.0")

    def test_react_native_tarball_must_not_carry_compiled_code(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            self.make_inventory(directory, "0.1.0")
            manifest = (directory / release_metadata.NATIVE_MANIFEST).read_bytes()
            self.make_react_native_tarball(
                directory,
                "0.1.0",
                manifest,
                extra={
                    "package/android/src/main/jniLibs/arm64-v8a/libkaleidorg_swap_sdk.so": b"\x7fELF"
                },
            )
            with self.assertRaisesRegex(ValueError, "compiled code"):
                assemble_release.collect_artifacts(directory, "0.1.0")

    def test_native_archive_missing_a_slice_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            self.make_inventory(directory, "0.1.0")
            manifest = self.make_native_archives(
                directory, "0.1.0", omit=next(iter(sorted(react_native.IOS_LIBRARIES)))
            )
            self.make_react_native_tarball(directory, "0.1.0", manifest)
            with self.assertRaisesRegex(ValueError, "inventory does not match"):
                assemble_release.collect_artifacts(directory, "0.1.0")

    def test_missing_platform_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            self.make_inventory(directory, "0.1.0")
            next(directory.glob("*win_amd64.whl")).unlink()
            with self.assertRaisesRegex(ValueError, r"expected \d+ wheels"):
                assemble_release.collect_artifacts(directory, "0.1.0")

    def test_invalid_npm_archive_is_rejected_before_release_ready(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            self.make_inventory(directory, "0.1.0")
            npm = directory / NPM_TARBALL_0_1_0
            with tarfile.open(npm, "w:gz") as archive:
                package = json.dumps({"name": NPM_PACKAGE, "version": "0.1.0"}).encode()
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

    def test_sealed_bundle_rechecks_the_native_binding(self) -> None:
        # Checksums prove the bytes; the binding proves they install together.
        # The verifier must do both from the sealed bundle.
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            self.make_release_bundle(directory)
            with mock.patch.object(verify_bundle, "verify_native_binding") as binding:
                verify_bundle.verify(
                    directory,
                    version="0.1.0",
                    tag="v0.1.0",
                    commit="HEAD",
                )
        binding.assert_called_once_with(directory, "0.1.0")

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
            with self.assertRaisesRegex(
                ValueError,
                f"expected {release_metadata.RELEASE_ASSET_COUNT} release assets",
            ):
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

    def test_release_tag_must_be_reachable_from_trunk(self) -> None:
        # The twin of the rehearsal test below, and the reason both exist: the
        # two directions are opposite and both correct, so a swapped argument
        # pair would check "trunk is reachable from the tag" — letting an
        # unreachable tag publish. This pins the release direction.
        with (
            mock.patch.object(release_ref, "validate_tag"),
            mock.patch.object(
                release_ref,
                "git",
                side_effect=["source-commit", "source-commit", "trunk-commit"],
            ),
            mock.patch.object(release_ref, "require_ancestor") as require_ancestor,
        ):
            release_ref.validate_release_ref(
                "v0.1.0",
                "source-sha",
                "origin/trunk",
            )
        require_ancestor.assert_called_once_with(
            "source-commit",
            "trunk-commit",
            "source-commit is not reachable from origin/trunk",
        )

    def test_release_ref_rejects_tag_pointing_elsewhere(self) -> None:
        with (
            mock.patch.object(release_ref, "validate_tag"),
            mock.patch.object(
                release_ref,
                "git",
                side_effect=["other-commit", "source-commit", "trunk-commit"],
            ),
            self.assertRaisesRegex(ValueError, "but the workflow is building"),
        ):
            release_ref.validate_release_ref("v0.1.0", "source-sha", "origin/trunk")

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

    def test_unexpected_secret_is_rejected(self) -> None:
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        with self.assertRaisesRegex(ValueError, "unexpected secrets"):
            workflow.validate(contents + "\n# secrets.SOME_OTHER_TOKEN\n")

    def test_registry_secret_outside_release_environment_is_rejected(self) -> None:
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        changed = contents.replace(
            "  verify-npm:\n",
            "  verify-npm:\n    env:\n      LEAK: ${{ secrets.NPM_TOKEN }}\n",
            1,
        )
        with self.assertRaisesRegex(ValueError, "without the protected release"):
            workflow.validate(changed)

    def test_skip_existing_yaml_input_is_rejected(self) -> None:
        # The PyPA action takes skip-existing as a YAML input, not a CLI flag.
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        with self.assertRaisesRegex(ValueError, "never skip an existing version"):
            workflow.validate(contents + "\n          skip-existing: true\n")

    def test_npm_oidc_scope_requires_provenance_in_manifest(self) -> None:
        # The npm job's id-token scope is justified only by provenance; if the
        # manifest field goes, the scope must go with it.
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        manifest = ROOT / "typescript-sdk/package.json"
        original = manifest.read_text(encoding="utf-8")
        payload = json.loads(original)
        payload["publishConfig"].pop("provenance", None)
        try:
            manifest.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "publishConfig.provenance"):
                workflow.validate(contents)
        finally:
            manifest.write_text(original, encoding="utf-8")

    def test_react_native_manifest_must_also_claim_provenance(self) -> None:
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        manifest = ROOT / "packages/react-native/package.json"
        original = manifest.read_text(encoding="utf-8")
        payload = json.loads(original)
        payload["publishConfig"].pop("provenance", None)
        try:
            manifest.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "packages/react-native/package.json"):
                workflow.validate(contents)
        finally:
            manifest.write_text(original, encoding="utf-8")

    def test_basic_auth_is_rejected(self) -> None:
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        with self.assertRaisesRegex(ValueError, "not basic auth"):
            workflow.validate(contents + "\n# username: someone\n")

    def test_npm_publish_without_a_path_prefix_is_rejected(self) -> None:
        # This shipped: v0.1.0 published to PyPI and then died here with code
        # 128, because npm read `release-artifacts/...tgz` as a GitHub
        # `owner/repo` shorthand and went looking for a git remote. The
        # rehearsal cannot catch it — it has no publish step at all.
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        regressed = contents.replace(
            "npm publish ./release-artifacts/kaleidorg-swap-sdk-${VERSION}.tgz",
            "npm publish release-artifacts/kaleidorg-swap-sdk-${VERSION}.tgz",
            1,
        )
        self.assertNotEqual(regressed, contents)
        with self.assertRaisesRegex(ValueError, "explicit file path"):
            workflow.validate(regressed)

    def test_npm_publish_path_is_accepted(self) -> None:
        workflow.validate((ROOT / ".github/workflows/release.yaml").read_text())

    def test_both_npm_tarballs_must_be_published(self) -> None:
        # One bundle, two packages. Dropping the second line ships the browser
        # package and leaves the release page pointing at archives no tarball
        # can fetch.
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        line = (
            "          npm publish ./release-artifacts/"
            "kaleidorg-swap-sdk-react-native-${VERSION}.tgz --access public\n"
        )
        self.assertIn(line, contents)
        with self.assertRaisesRegex(ValueError, "both npm tarballs"):
            workflow.validate(contents.replace(line, "", 1))

    def test_react_native_install_must_follow_the_github_release(self) -> None:
        # postinstall fetches from the release, so the proof that a partner's
        # install works can only run once the release exists.
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        changed = contents.replace(
            "  verify-react-native-install:\n"
            "    name: Verify the React Native package installs from the registry and the release\n"
            "    if: ${{ vars.NPM_PUBLISH_ENABLED == 'true' }}\n"
            "    needs:\n"
            "      - release-ready\n"
            "      - publish-github-release\n"
            "      - registry-publish-complete\n",
            "  verify-react-native-install:\n"
            "    name: Verify the React Native package installs from the registry and the release\n"
            "    if: ${{ vars.NPM_PUBLISH_ENABLED == 'true' }}\n"
            "    needs:\n"
            "      - release-ready\n"
            "      - registry-publish-complete\n",
            1,
        )
        self.assertNotEqual(changed, contents)
        with self.assertRaisesRegex(ValueError, "after the GitHub release"):
            workflow.validate(changed)

    def test_npm_publish_must_wait_for_the_complete_github_release(self) -> None:
        # postinstall fetches native archives from the matching GitHub release.
        # Publishing npm first creates an unfixable broken-version window, and
        # the wait step is now the only thing that prevents it.
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        npm_job = workflow.production_jobs(contents)["publish-npm"]
        changed = contents.replace(
            npm_job,
            npm_job.replace(
                "      - name: Wait for the complete GitHub release\n",
                "      - name: Wait a moment\n",
                1,
            ),
            1,
        )
        self.assertNotEqual(changed, contents)
        with self.assertRaisesRegex(
            ValueError, "npm publisher must wait for the complete GitHub release"
        ):
            workflow.validate(changed)

    def test_pypi_publish_must_wait_for_the_complete_github_release(self) -> None:
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        pypi_job = workflow.production_jobs(contents)["publish-pypi"]
        changed = contents.replace(
            pypi_job,
            pypi_job.replace('select(.state == "uploaded")', ".", 1),
            1,
        )
        self.assertNotEqual(changed, contents)
        with self.assertRaisesRegex(
            ValueError, "PyPI publisher must wait for the complete GitHub release"
        ):
            workflow.validate(changed)

    def test_npm_publish_after_the_wait_is_required(self) -> None:
        # A wait that runs after the publish holds nothing.
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        npm_job = workflow.production_jobs(contents)["publish-npm"]
        wait_start = npm_job.index("      - name: Wait for the complete GitHub release")
        wait_end = npm_job.index("      # The `./` is load-bearing.")
        wait_step = npm_job[wait_start:wait_end]
        reordered = npm_job[:wait_start] + npm_job[wait_end:] + wait_step
        changed = contents.replace(npm_job, reordered, 1)
        self.assertNotEqual(changed, contents)
        with self.assertRaisesRegex(ValueError, "before it publishes"):
            workflow.validate(changed)

    def test_publisher_needing_the_github_release_is_rejected(self) -> None:
        # A job that reaches the release environment after another publisher
        # was approved asks for a second approval, and the gap between the two
        # is a public release with no package. Anchored on the job header.
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        changed = contents.replace(
            "  publish-npm:\n"
            "    name: Publish exact npm tarball\n"
            "    if: ${{ vars.NPM_PUBLISH_ENABLED == 'true' }}\n"
            "    needs: release-ready\n",
            "  publish-npm:\n"
            "    name: Publish exact npm tarball\n"
            "    if: ${{ vars.NPM_PUBLISH_ENABLED == 'true' }}\n"
            "    needs:\n"
            "      - release-ready\n"
            "      - publish-github-release\n",
            1,
        )
        self.assertNotEqual(changed, contents)
        with self.assertRaisesRegex(ValueError, "same single approval"):
            workflow.validate(changed)

    def test_github_release_must_be_reviewed(self) -> None:
        # The release is the first public trace of a version, so it sits behind
        # the same review as the registries.
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        release_job = workflow.production_jobs(contents)["publish-github-release"]
        changed = contents.replace(
            release_job,
            release_job.replace("    environment: release\n", "", 1),
            1,
        )
        self.assertNotEqual(changed, contents)
        with self.assertRaisesRegex(
            ValueError, "GitHub release must use the release environment"
        ):
            workflow.validate(changed)

    def test_github_release_must_not_wait_on_a_second_approval(self) -> None:
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        changed = contents.replace(
            "  publish-github-release:\n"
            "    name: Publish final GitHub release\n"
            "    needs: release-ready\n",
            "  publish-github-release:\n"
            "    name: Publish final GitHub release\n"
            "    needs:\n"
            "      - release-ready\n"
            "      - release-activation\n",
            1,
        )
        self.assertNotEqual(changed, contents)
        with self.assertRaisesRegex(ValueError, "same single approval"):
            workflow.validate(changed)

    def test_github_release_must_not_wait_for_registry_completion(self) -> None:
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        changed = contents.replace(
            "  publish-github-release:\n"
            "    name: Publish final GitHub release\n"
            "    needs: release-ready\n",
            "  publish-github-release:\n"
            "    name: Publish final GitHub release\n"
            "    needs:\n"
            "      - release-ready\n"
            "      - registry-publish-complete\n",
            1,
        )
        self.assertNotEqual(changed, contents)
        with self.assertRaisesRegex(ValueError, "must precede registry publication"):
            workflow.validate(changed)

    def test_runbook_publication_order_must_match_the_workflow(self) -> None:
        # The runbook is read during an incident. Prose that outlives the job
        # graph it describes is worse than no prose, because it is trusted.
        runbook = (ROOT / "docs/releasing.md").read_text()
        changed = runbook.replace(
            "2. `release-ready`\n3. `publish-github-release`",
            "2. `publish-github-release`\n3. `release-ready`",
            1,
        )
        self.assertNotEqual(changed, runbook)
        with self.assertRaisesRegex(ValueError, "contradicts the"):
            workflow.validate(
                (ROOT / ".github/workflows/release.yaml").read_text(),
                runbook_contents=changed,
            )

    def test_runbook_must_list_every_production_job(self) -> None:
        runbook = (ROOT / "docs/releasing.md").read_text()
        changed = runbook.replace("9. `verify-react-native-install`\n", "", 1)
        self.assertNotEqual(changed, runbook)
        with self.assertRaisesRegex(ValueError, "must list every production job"):
            workflow.validate(
                (ROOT / ".github/workflows/release.yaml").read_text(),
                runbook_contents=changed,
            )

    def test_runbook_must_carry_a_publication_order(self) -> None:
        runbook = (ROOT / "docs/releasing.md").read_text()
        changed = runbook.replace("## Publication order", "## Order", 1)
        self.assertNotEqual(changed, runbook)
        with self.assertRaisesRegex(ValueError, "'Publication order' section"):
            workflow.validate(
                (ROOT / ".github/workflows/release.yaml").read_text(),
                runbook_contents=changed,
            )

    def test_react_native_install_must_run_postinstall(self) -> None:
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        changed = contents.replace(
            "smoke-react-native-install.mjs\n",
            "smoke-react-native-install.mjs --ignore-scripts\n",
            1,
        )
        self.assertNotEqual(changed, contents)
        with self.assertRaisesRegex(ValueError, "must not disable the postinstall"):
            workflow.validate(changed)

    def test_composite_action_is_held_to_build_rules(self) -> None:
        # The build workflow calls the action with its own authority, so a
        # mutable ref or a publish step inside it is as bad as one in the
        # workflow file.
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        with self.assertRaisesRegex(ValueError, "mutable"):
            workflow.validate(
                contents, actions_contents="\n      uses: actions/cache@v4\n"
            )
        with self.assertRaisesRegex(ValueError, "release authority"):
            workflow.validate(contents, actions_contents="\n      run: npm publish .\n")

    def test_extra_oidc_permission_is_rejected(self) -> None:
        # A real permission key, not a comment: the count is anchored to YAML
        # keys so that a comment mentioning the scope cannot inflate it.
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        with self.assertRaisesRegex(ValueError, "exactly the npm publish job"):
            workflow.validate(contents + "\n      id-token: write\n")

    def test_commented_oidc_scope_does_not_count(self) -> None:
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        workflow.validate(contents + "\n# id-token: write\n")

    def test_pypi_publisher_must_not_request_oidc(self) -> None:
        # PEP 740 attestations need Trusted Publishing, so under token auth an
        # id-token scope on the PyPI job is unused privilege.
        contents = (ROOT / ".github/workflows/release.yaml").read_text()
        changed = contents.replace(
            "  publish-pypi:\n    name: Publish exact Python artifacts to PyPI\n",
            "  publish-pypi:\n    name: Publish exact Python artifacts to PyPI\n"
            "    permissions:\n      id-token: write\n",
            1,
        )
        with self.assertRaises(ValueError):
            workflow.validate(changed)

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
        with self.assertRaisesRegex(ValueError, "must download registry artifacts"):
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


class WorkflowExpressionTests(unittest.TestCase):
    """Evaluate the release-build env expressions under Actions' falsy rules.

    GitHub Actions returns operand values from `&&`/`||` and treats "" as falsy,
    so `a && b || c` silently substitutes `c` whenever `b` is empty. An empty
    release_tag is meaningful here, so it must not pass through such a fallback.
    """

    FALSY = {"", "0", "false", "null"}

    @classmethod
    def evaluate(cls, expression: str, context: dict[str, str]) -> str:
        """Evaluate a chain of `&&`/`||` over context lookups and literals."""
        tokens = re.split(r"\s+(&&|\|\|)\s+", expression.strip())

        def value(token: str) -> str:
            token = token.strip()
            literal = re.fullmatch(r"'([^']*)'", token)
            if literal:
                return literal.group(1)
            if token not in context:
                raise AssertionError(f"unhandled operand {token!r}")
            return context[token]

        result = value(tokens[0])
        for operator, operand in zip(tokens[1::2], tokens[2::2]):
            truthy = result not in cls.FALSY
            if operator == "&&":
                result = value(operand) if truthy else result
            else:
                result = result if truthy else value(operand)
        return result

    def env_expression(self, name: str) -> str:
        build = (ROOT / ".github/workflows/release-build.yaml").read_text()
        match = re.search(
            rf"^  {re.escape(name)}: \$\{{\{{(.+?)\}}\}}$", build, flags=re.MULTILINE
        )
        assert match is not None, f"{name} is not a single-line expression"
        return match.group(1)

    def test_empty_rehearsal_tag_survives_to_the_script(self) -> None:
        result = self.evaluate(
            self.env_expression("RELEASE_TAG"),
            {
                "inputs.rehearsal": "true",
                "inputs.release_tag": "",
                "github.ref_name": "9/merge",
            },
        )
        self.assertEqual(
            result,
            "",
            "an empty rehearsal release_tag must reach preflight so it can derive "
            "the tag; a `||` fallback would substitute the pull-request ref",
        )

    def test_production_tag_is_passed_through(self) -> None:
        result = self.evaluate(
            self.env_expression("RELEASE_TAG"),
            {
                "inputs.rehearsal": "false",
                "inputs.release_tag": "v0.1.0",
                "github.ref_name": "v0.1.0",
            },
        )
        self.assertEqual(result, "v0.1.0")


class RuntimeVersionTests(unittest.TestCase):
    def test_python_package_reports_a_version_without_a_second_source(self) -> None:
        source = (ROOT / "bindings/python/kaleidorg_swap_sdk/__init__.py").read_text()
        self.assertIn("__version__", source)
        # Derived from installed metadata, so it can never drift from pyproject.
        self.assertIn("_distribution_version", source)
        self.assertNotIn('__version__ = "0.1.0"', source)


if __name__ == "__main__":
    unittest.main()
