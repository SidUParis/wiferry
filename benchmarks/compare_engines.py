#!/usr/bin/env python3
"""Reproducible loopback comparison of the legacy and Rust host engines."""

from __future__ import annotations

import argparse
import hashlib
import json
import platform
import socket
import statistics
import subprocess
import tempfile
import time
import urllib.request
from pathlib import Path


def free_port() -> int:
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        return probe.getsockname()[1]


def process_tree_rss_bytes(root_pid: int) -> int | None:
    processes: dict[int, tuple[int, int]] = {}
    for status in Path("/proc").glob("[0-9]*/status"):
        try:
            fields = {}
            for line in status.read_text().splitlines():
                if line.startswith(("Pid:", "PPid:", "VmRSS:")):
                    key, value = line.split(":", 1)
                    fields[key] = int(value.split()[0])
            processes[fields["Pid"]] = (fields["PPid"], fields.get("VmRSS", 0) * 1024)
        except (OSError, KeyError, ValueError):
            continue
    family = {root_pid}
    changed = True
    while changed:
        changed = False
        for pid, (parent, _) in processes.items():
            if parent in family and pid not in family:
                family.add(pid)
                changed = True
    values = [processes[pid][1] for pid in family if pid in processes]
    return sum(values) if values else None


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def run_once(executable: Path, source: Path) -> dict[str, float | int | str | None]:
    port = free_port()
    started = time.perf_counter()
    process = subprocess.Popen(
        [
            str(executable),
            "--no-browser",
            "--host-ip",
            "127.0.0.1",
            "--port",
            str(port),
            "--name",
            "Benchmark",
            "--file",
            str(source),
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    try:
        guest_url = ""
        for _ in range(8):
            line = process.stdout.readline().strip()
            if line.startswith("Nearby devices:"):
                guest_url = line.split("Nearby devices:", 1)[1].strip()
                break
        if not guest_url:
            raise RuntimeError("Engine did not print a nearby-device URL")
        token = guest_url.rstrip("/").rsplit("/", 1)[1]
        state_url = f"http://127.0.0.1:{port}/api/session/{token}"
        deadline = time.time() + 15
        while True:
            try:
                with urllib.request.urlopen(state_url, timeout=1) as response:
                    state = json.load(response)
                break
            except OSError:
                if time.time() >= deadline:
                    raise
                time.sleep(0.01)
        ready_ms = (time.perf_counter() - started) * 1000
        time.sleep(0.1)
        rss = process_tree_rss_bytes(process.pid)
        file_id = state["files"][0]["id"]
        download_url = f"{state_url}/files/{file_id}"
        digest = hashlib.sha256()
        transfer_started = time.perf_counter()
        with urllib.request.urlopen(download_url, timeout=60) as response:
            while chunk := response.read(1024 * 1024):
                digest.update(chunk)
        elapsed = time.perf_counter() - transfer_started
        expected = sha256_file(source)
        if digest.hexdigest() != expected:
            raise RuntimeError("Downloaded checksum differs from source")
        return {
            "ready_ms": ready_ms,
            "idle_rss_bytes": rss,
            "download_seconds": elapsed,
            "throughput_mib_s": source.stat().st_size / (1024 * 1024) / elapsed,
            "binary_bytes": executable.stat().st_size,
            "binary_sha256": sha256_file(executable),
            "sha256": digest.hexdigest(),
        }
    finally:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=2)


def summarize(name: str, samples: list[dict]) -> dict:
    def median(field: str):
        values = [sample[field] for sample in samples if sample[field] is not None]
        return statistics.median(values) if values else None

    return {
        "engine": name,
        "runs": len(samples),
        "binary_bytes": samples[0]["binary_bytes"],
        "binary_sha256": samples[0]["binary_sha256"],
        "ready_ms_median": median("ready_ms"),
        "idle_rss_bytes_median": median("idle_rss_bytes"),
        "throughput_mib_s_median": median("throughput_mib_s"),
        "download_seconds_median": median("download_seconds"),
        "checksum": samples[0]["sha256"],
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--legacy", type=Path, required=True)
    parser.add_argument("--rust", type=Path, required=True)
    parser.add_argument("--size-mib", type=int, default=64)
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--json-out", type=Path)
    parser.add_argument("--source-revision")
    args = parser.parse_args()

    with tempfile.TemporaryDirectory(prefix="wiferry-benchmark-") as temp:
        source = Path(temp) / "benchmark payload.bin"
        block = bytes(range(256)) * 4096
        with source.open("wb") as output:
            for _ in range(args.size_mib):
                output.write(block)
        engines = (("legacy-python", args.legacy), ("rust", args.rust))
        samples_by_engine = {name: [] for name, _ in engines}
        execution_order = []
        for run_number in range(1, args.runs + 1):
            ordered = engines if run_number % 2 else tuple(reversed(engines))
            for name, executable in ordered:
                sample = run_once(executable.resolve(), source)
                sample["run"] = run_number
                samples_by_engine[name].append(sample)
                execution_order.append(f"{run_number}:{name}")
        results = {
            name: summarize(name, samples_by_engine[name]) for name, _ in engines
        }
        payload = {
            "method": {
                "transport": "loopback HTTP",
                "file_size_mib": args.size_mib,
                "runs": args.runs,
                "chunk_read_mib": 1,
                "source_revision": args.source_revision,
                "host_platform": platform.platform(),
                "host_machine": platform.machine(),
                "python_version": platform.python_version(),
                "execution_order": execution_order,
                "note": "Loopback measures framework ceiling, not real Wi-Fi speed.",
            },
            # Keep `results` as the summary map so existing reports and readers
            # remain compatible; new runs also retain every raw observation.
            "results": results,
            "samples": samples_by_engine,
        }
        rendered = json.dumps(payload, indent=2)
        print(rendered)
        if args.json_out:
            args.json_out.parent.mkdir(parents=True, exist_ok=True)
            args.json_out.write_text(rendered + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
