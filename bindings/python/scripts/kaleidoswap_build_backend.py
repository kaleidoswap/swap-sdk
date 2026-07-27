"""PEP 517 backend that makes Maturin's UniFFI wheel layout deterministic."""

from __future__ import annotations

import base64
import csv
import hashlib
import io
import sys
import zipfile
from collections.abc import Mapping
from pathlib import Path
from typing import Any

PACKAGE_BINDINGS = "kaleidoswap_sdk/kaleidoswap_sdk/kaleidoswap_sdk.py"
PACKAGE_INIT = "kaleidoswap_sdk/kaleidoswap_sdk/__init__.py"
GENERATED_FILES = {
    PACKAGE_BINDINGS: Path("_generated/kaleidoswap_sdk.py"),
    PACKAGE_INIT: Path("_generated/__init__.py"),
}


def _project_root() -> Path:
    return Path(__file__).resolve().parent.parent


def _record_value(data: bytes) -> tuple[str, str]:
    digest = base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b"=")
    return f"sha256={digest.decode()}", str(len(data))


def repair_wheel(wheel: Path) -> bool:
    """Inject missing generated UniFFI glue and update the wheel RECORD."""
    with zipfile.ZipFile(wheel) as archive:
        entries = {
            info.filename: (info, archive.read(info)) for info in archive.infolist()
        }

    missing = [name for name in GENERATED_FILES if name not in entries]
    if not missing:
        return False

    record_names = [name for name in entries if name.endswith(".dist-info/RECORD")]
    if len(record_names) != 1:
        message = (
            f"{wheel}: expected exactly one .dist-info/RECORD, found {record_names}"
        )
        raise RuntimeError(message)
    record_name = record_names[0]

    generated = {
        name: (_project_root() / source).read_bytes()
        for name, source in GENERATED_FILES.items()
        if name in missing
    }

    rows = list(csv.reader(io.StringIO(entries[record_name][1].decode())))
    rows = [row for row in rows if row and row[0] not in generated]
    for name, data in generated.items():
        digest, size = _record_value(data)
        rows.append([name, digest, size])

    record_buffer = io.StringIO(newline="")
    csv.writer(record_buffer, lineterminator="\n").writerows(rows)
    entries[record_name] = (entries[record_name][0], record_buffer.getvalue().encode())

    temporary = wheel.with_suffix(".whl.tmp")
    try:
        with zipfile.ZipFile(temporary, "w") as archive:
            for info, data in entries.values():
                archive.writestr(info, data)
            for name, data in generated.items():
                info = zipfile.ZipInfo(name, entries[record_name][0].date_time)
                info.compress_type = zipfile.ZIP_DEFLATED
                info.create_system = 3
                info.external_attr = 0o100644 << 16
                archive.writestr(info, data)
        temporary.replace(wheel)
    finally:
        temporary.unlink(missing_ok=True)

    return True


def build_wheel(
    wheel_directory: str,
    config_settings: Mapping[str, Any] | None = None,
    metadata_directory: str | None = None,
) -> str:
    import maturin

    filename = maturin.build_wheel(
        wheel_directory,
        config_settings=config_settings,
        metadata_directory=metadata_directory,
    )
    repair_wheel(Path(wheel_directory) / filename)
    return filename


def get_requires_for_build_wheel(
    config_settings: Mapping[str, Any] | None = None,
) -> list[str]:
    import maturin

    return maturin.get_requires_for_build_wheel(config_settings=config_settings)


def build_sdist(
    sdist_directory: str,
    config_settings: Mapping[str, Any] | None = None,
) -> str:
    import maturin

    return maturin.build_sdist(sdist_directory, config_settings=config_settings)


def get_requires_for_build_sdist(
    config_settings: Mapping[str, Any] | None = None,
) -> list[str]:
    import maturin

    return maturin.get_requires_for_build_sdist(config_settings=config_settings)


def prepare_metadata_for_build_wheel(
    metadata_directory: str,
    config_settings: Mapping[str, Any] | None = None,
) -> str:
    import maturin

    return maturin.prepare_metadata_for_build_wheel(
        metadata_directory,
        config_settings=config_settings,
    )


def prepare_metadata_for_build_editable(
    metadata_directory: str,
    config_settings: Mapping[str, Any] | None = None,
) -> str:
    import maturin

    return maturin.prepare_metadata_for_build_editable(
        metadata_directory,
        config_settings=config_settings,
    )


def build_editable(
    wheel_directory: str,
    config_settings: Mapping[str, Any] | None = None,
    metadata_directory: str | None = None,
) -> str:
    import maturin

    return maturin.build_editable(
        wheel_directory,
        config_settings=config_settings,
        metadata_directory=metadata_directory,
    )


def get_requires_for_build_editable(
    config_settings: Mapping[str, Any] | None = None,
) -> list[str]:
    import maturin

    return maturin.get_requires_for_build_editable(config_settings=config_settings)


def _wheel_paths(path: Path) -> list[Path]:
    if path.is_dir():
        return sorted(path.glob("*.whl"))
    if path.suffix == ".whl":
        return [path]
    return []


def main(arguments: list[str]) -> int:
    if not arguments:
        print(
            "usage: kaleidoswap_build_backend.py WHEEL_OR_DIRECTORY [...]",
            file=sys.stderr,
        )
        return 2

    wheels = [wheel for argument in arguments for wheel in _wheel_paths(Path(argument))]
    if not wheels:
        print("no wheels found", file=sys.stderr)
        return 1

    for wheel in wheels:
        action = "repaired" if repair_wheel(wheel) else "verified"
        print(f"{action}: {wheel}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
