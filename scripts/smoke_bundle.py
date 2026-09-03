#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

import tomllib


def free_port() -> int:
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        return probe.getsockname()[1]


def require(condition: bool, detail: str) -> None:
    if not condition:
        raise RuntimeError(detail)


def main() -> None:
    executable = Path(sys.argv[1]).resolve()
    with (Path(__file__).resolve().parents[1] / "Cargo.toml").open("rb") as source:
        version = tomllib.load(source)["package"]["version"]
    port = free_port()
    with tempfile.TemporaryDirectory(prefix="wiferry-bundle-smoke-") as temp:
        source = Path(temp) / "release smoke 你好 café.bin"
        prefix = b"Wiferry cross-platform bundle smoke test.\n"
        expected = prefix + bytes(range(256)) * 1_537
        source.write_bytes(expected)
        process = subprocess.Popen(
            [
                str(executable),
                "--no-browser",
                "--host-ip",
                "127.0.0.1",
                "--port",
                str(port),
                "--name",
                "Bundle QA",
                str(source),
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )
        try:
            if process.stdout is None:
                raise RuntimeError("Could not capture bundled server output")
            lines = [process.stdout.readline().strip() for _ in range(3)]
            nearby_line = next(
                (line for line in lines if line.startswith("Nearby devices:")), None
            )
            if nearby_line is None:
                raise RuntimeError("Bundled server did not print a nearby-device URL:\n" + "\n".join(lines))
            guest_url = nearby_line.split("Nearby devices:", 1)[1].strip()
            token = guest_url.rstrip("/").rsplit("/", 1)[1]
            deadline = time.time() + 20
            while time.time() < deadline:
                if process.poll() is not None:
                    raise RuntimeError("\n".join(lines) + process.stdout.read())
                try:
                    with urllib.request.urlopen(
                        f"http://127.0.0.1:{port}/api/session/{token}", timeout=1
                    ) as response:
                        state = json.load(response)
                    break
                except OSError:
                    time.sleep(0.25)
            else:
                raise RuntimeError("Bundled server did not start within 20 seconds")

            require(state["deviceName"] == "Bundle QA", "Unexpected device name")
            require(state["files"][0]["name"] == source.name, "Unicode/space filename changed")
            require(state["files"][0]["size"] == len(expected), "Shared file size changed")
            file_id = state["files"][0]["id"]
            download_url = f"http://127.0.0.1:{port}/api/session/{token}/files/{file_id}"
            with urllib.request.urlopen(download_url, timeout=10) as response:
                require(response.status == 200, f"Full download returned HTTP {response.status}")
                downloaded = response.read()
            full_sha256 = hashlib.sha256(downloaded).hexdigest()
            require(
                full_sha256 == hashlib.sha256(expected).hexdigest(),
                "Full download SHA-256 mismatch",
            )

            range_start = 128 * 1024 - 17
            range_end = 3 * 128 * 1024 + 31
            range_request = urllib.request.Request(
                download_url,
                headers={"Range": f"bytes={range_start}-{range_end}"},
            )
            with urllib.request.urlopen(range_request, timeout=10) as response:
                require(response.status == 206, f"Range download returned HTTP {response.status}")
                require(response.headers["Accept-Ranges"] == "bytes", "Missing Accept-Ranges")
                require(
                    response.headers["Content-Range"]
                    == f"bytes {range_start}-{range_end}/{len(expected)}",
                    "Unexpected Content-Range",
                )
                ranged = response.read()
            expected_range = expected[range_start : range_end + 1]
            range_sha256 = hashlib.sha256(ranged).hexdigest()
            require(
                range_sha256 == hashlib.sha256(expected_range).hexdigest(),
                "Range download SHA-256 mismatch",
            )

            output = subprocess.check_output([str(executable), "--version"], text=True).strip()
            require(output == f"wiferry {version}", f"Unexpected binary version: {output}")
            print(
                json.dumps(
                    {
                        "startup": "ok",
                        "version": output,
                        "unicodeSpacePath": "ok",
                        "fullDownloadSha256": full_sha256,
                        "rangeDownloadSha256": range_sha256,
                    },
                    sort_keys=True,
                )
            )
        finally:
            process.terminate()
            try:
                process.wait(timeout=8)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=4)


if __name__ == "__main__":
    main()
