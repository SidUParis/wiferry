#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import platform
import shutil
import stat
import sys
import tarfile
import tempfile
import zipfile
from pathlib import Path

import tomllib

PROJECT_ROOT = Path(__file__).resolve().parents[1]


def project_version() -> str:
    with (PROJECT_ROOT / "Cargo.toml").open("rb") as source:
        return tomllib.load(source)["package"]["version"]


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def platform_name() -> str:
    if sys.platform.startswith("linux"):
        return "linux"
    if sys.platform == "darwin":
        return "macos"
    if sys.platform == "win32":
        return "windows"
    raise SystemExit(f"Unsupported release platform: {sys.platform}")


def architecture_name() -> str:
    machine = platform.machine().lower()
    aliases = {"amd64": "x86_64", "x64": "x86_64", "aarch64": "arm64"}
    return aliases.get(machine, machine)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("executable", type=Path)
    parser.add_argument("--output", type=Path, default=Path("release"))
    parser.add_argument("--expect-platform", choices=("linux", "macos", "windows"))
    parser.add_argument("--expect-arch", choices=("x86_64", "arm64"))
    args = parser.parse_args()

    executable = args.executable.resolve()
    if not executable.is_file():
        raise SystemExit(f"Executable not found: {executable}")
    system = platform_name()
    architecture = architecture_name()
    if args.expect_platform and system != args.expect_platform:
        raise SystemExit(
            f"Release runner platform mismatch: expected {args.expect_platform}, detected {system}"
        )
    if args.expect_arch and architecture != args.expect_arch:
        raise SystemExit(
            f"Release runner architecture mismatch: expected {args.expect_arch}, "
            f"detected {architecture}"
        )
    package_name = f"wiferry-{project_version()}-{system}-{architecture}"
    args.output.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory(prefix="wiferry-release-") as temp:
        root = Path(temp) / package_name
        root.mkdir()
        target_name = "wiferry.exe" if system == "windows" else "wiferry"
        target = root / target_name
        shutil.copy2(executable, target)
        if system != "windows":
            target.chmod(target.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP)
        for document in (
            "README.md",
            "README.zh-CN.md",
            "CHANGELOG.md",
            "LICENSE",
            "SECURITY.md",
        ):
            shutil.copy2(PROJECT_ROOT / document, root / document)

        if system == "windows":
            archive = args.output / f"{package_name}.zip"
            with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as bundle:
                for path in sorted(root.rglob("*")):
                    if path.is_file():
                        bundle.write(path, path.relative_to(root.parent))
        else:
            archive = args.output / f"{package_name}.tar.gz"
            with tarfile.open(archive, "w:gz") as bundle:
                bundle.add(root, arcname=root.name)

    checksum = sha256(archive)
    checksum_path = archive.with_suffix(archive.suffix + ".sha256")
    checksum_path.write_text(f"{checksum}  {archive.name}\n", encoding="utf-8")
    print(archive)
    print(checksum_path)


if __name__ == "__main__":
    main()
