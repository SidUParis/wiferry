#!/usr/bin/env python3
from __future__ import annotations

import os
from pathlib import Path

import tomllib


def main() -> None:
    if os.environ.get("GITHUB_REF_TYPE") != "tag":
        return
    with (Path(__file__).resolve().parents[1] / "Cargo.toml").open("rb") as source:
        version = tomllib.load(source)["package"]["version"]
    actual = os.environ.get("GITHUB_REF_NAME", "")
    expected = f"v{version}"
    if actual != expected:
        raise SystemExit(f"Release tag {actual!r} does not match project version {expected!r}")
    print(f"release tag verified: {actual}")


if __name__ == "__main__":
    main()
