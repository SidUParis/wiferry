from __future__ import annotations

import argparse
import threading
import webbrowser
from pathlib import Path

import uvicorn

from . import __version__
from .app import create_app
from .core import TransferSession, detect_lan_ip


def parser() -> argparse.ArgumentParser:
    command = argparse.ArgumentParser(
        prog="wiferry", description="Share files privately over the same Wi-Fi."
    )
    command.add_argument("paths", nargs="*", type=Path, help="Files to share immediately")
    command.add_argument(
        "-f",
        "--file",
        dest="files",
        action="append",
        type=Path,
        default=[],
        help="File to share; repeat this option for several files",
    )
    command.add_argument("--port", type=int, default=8765)
    command.add_argument("--host-ip", help="LAN address encoded into the QR code")
    command.add_argument("--receive-dir", type=Path, help="Where received files are saved")
    command.add_argument("--name", help="Friendly device name shown to nearby browsers")
    command.add_argument("--expiry", type=int, default=30, choices=[0, 15, 30, 60, 120])
    command.add_argument("--receive", action="store_true", help="Start in receive mode")
    command.add_argument("--no-browser", action="store_true")
    command.add_argument("--version", action="version", version=f"%(prog)s {__version__}")
    return command


def main() -> None:
    argument_parser = parser()
    args = argument_parser.parse_args()
    session = TransferSession(
        host_ip=args.host_ip or detect_lan_ip(),
        port=args.port,
        expiry_minutes=args.expiry,
        receive_dir=args.receive_dir,
        device_name=args.name,
    )
    if args.receive:
        session.set_mode("receive")
    for raw_path in [*args.paths, *args.files]:
        path = raw_path.expanduser()
        if not path.exists():
            argument_parser.error(f"file does not exist: {raw_path}")
        if not path.is_file():
            argument_parser.error(f"directories are not supported yet: {raw_path}")
        session.add_path(path)

    admin_url = f"http://127.0.0.1:{args.port}/"
    print(f"Wiferry management: {admin_url}", flush=True)
    print(f"Nearby devices:     {session.share_url}", flush=True)
    print("Press Ctrl+C to stop sharing.", flush=True)
    if not args.no_browser:
        threading.Timer(0.8, lambda: webbrowser.open(admin_url)).start()

    try:
        uvicorn.run(
            create_app(session),
            host="0.0.0.0",
            port=args.port,
            log_level="warning",
            lifespan="off",
        )
    finally:
        session.close()
