from __future__ import annotations

import ipaddress
import mimetypes
import os
import re
import secrets
import socket
import tempfile
import threading
import time
import unicodedata
import uuid
from collections import OrderedDict
from collections.abc import Callable
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import BinaryIO

WINDOWS_RESERVED = {
    "CON",
    "PRN",
    "AUX",
    "NUL",
    *(f"COM{i}" for i in range(1, 10)),
    *(f"LPT{i}" for i in range(1, 10)),
}


class TransferRevoked(RuntimeError):
    pass


class SessionAccessError(RuntimeError):
    def __init__(self, status: int, detail: str) -> None:
        super().__init__(detail)
        self.status = status
        self.detail = detail


def _truncate_utf8(value: str, budget: int) -> str:
    encoded = value.encode("utf-8")
    if len(encoded) <= budget:
        return value
    return encoded[:budget].decode("utf-8", "ignore")


def _routed_ip() -> str | None:
    probe = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        probe.connect(("1.1.1.1", 80))
        return probe.getsockname()[0]
    except OSError:
        return None
    finally:
        probe.close()


def discover_lan_ips() -> list[str]:
    """Find plausible IPv4 addresses and put the OS default route first."""
    candidates: list[str] = []

    def add(address: str | None) -> None:
        if not address:
            return
        try:
            parsed = ipaddress.ip_address(address)
        except ValueError:
            return
        if (
            parsed.version != 4
            or parsed.is_loopback
            or parsed.is_link_local
            or parsed.is_multicast
            or parsed.is_unspecified
        ):
            return
        if address not in candidates:
            candidates.append(address)

    add(_routed_ip())
    for host in {socket.gethostname(), socket.getfqdn()}:
        try:
            for result in socket.getaddrinfo(host, None, socket.AF_INET, socket.SOCK_DGRAM):
                add(result[4][0])
        except OSError:
            continue
    return candidates


def detect_lan_ip() -> str:
    """Return the best available address for the QR code."""
    candidates = discover_lan_ips()
    return candidates[0] if candidates else "127.0.0.1"


def safe_filename(name: str) -> str:
    normalized = unicodedata.normalize("NFC", Path(name.replace("\\", "/")).name)
    normalized = re.sub(r'[<>:"/\\|?*\x00-\x1f]', "_", normalized).strip(" .")
    if normalized in {"", ".", ".."}:
        normalized = "shared-file"
    if normalized.split(".", 1)[0].upper() in WINDOWS_RESERVED:
        normalized = f"_{normalized}"
    suffix = Path(normalized).suffix
    if len(suffix.encode("utf-8")) > 32:
        suffix = ""
    suffix_bytes = suffix.encode("utf-8")
    budget = 240
    stem_budget = max(1, budget - len(suffix_bytes))
    stem = normalized[: -len(suffix)] if suffix else normalized
    stem = _truncate_utf8(stem, stem_budget)
    return _truncate_utf8(f"{stem}{suffix}", budget) or "shared-file"


def human_size(size: int) -> str:
    value = float(size)
    for unit in ("B", "KB", "MB", "GB", "TB"):
        if value < 1024 or unit == "TB":
            return f"{value:.0f} {unit}" if unit in {"B", "KB"} else f"{value:.1f} {unit}"
        value /= 1024
    return f"{size} B"


@dataclass
class SharedFile:
    id: str
    name: str
    path: Path
    size: int
    mime: str
    owned_copy: bool = False
    readers: int = 0
    pending_delete: bool = False

    def public(self) -> dict[str, object]:
        return {
            "id": self.id,
            "name": self.name,
            "size": self.size,
            "sizeLabel": human_size(self.size),
            "mime": self.mime,
        }


@dataclass
class Activity:
    id: str
    kind: str
    name: str
    size: int
    device: str
    status: str
    timestamp: float

    def public(self) -> dict[str, object]:
        data = asdict(self)
        data["sizeLabel"] = human_size(self.size)
        return data


class TransferSession:
    def __init__(
        self,
        *,
        host_ip: str,
        port: int,
        expiry_minutes: int = 30,
        receive_dir: Path | None = None,
        device_name: str | None = None,
    ) -> None:
        self.host_ip = host_ip
        self.host_candidates = discover_lan_ips()
        if host_ip not in self.host_candidates:
            self.host_candidates.insert(0, host_ip)
        self.port = port
        self.device_name = device_name or socket.gethostname().split(".", 1)[0]
        self.expiry_minutes = expiry_minutes
        self.token = secrets.token_urlsafe(24)
        self.admin_token = secrets.token_urlsafe(24)
        self.mode = "send"
        self.active = True
        self.created_at = time.time()
        self.expires_at = self.created_at + expiry_minutes * 60
        self.files: OrderedDict[str, SharedFile] = OrderedDict()
        self.activities: list[Activity] = []
        self.clients: dict[str, float] = {}
        self.generation = 0
        self.lock = threading.RLock()
        self._temp = tempfile.TemporaryDirectory(prefix="wiferry-")
        self.share_dir = Path(self._temp.name) / "shared"
        self.share_dir.mkdir(parents=True, exist_ok=True)
        self.receive_dir = (receive_dir or Path.home() / "Downloads" / "Wiferry").resolve()
        self.receive_dir.mkdir(parents=True, exist_ok=True)

    @property
    def share_url(self) -> str:
        return f"http://{self.host_ip}:{self.port}/s/{self.token}/"

    def set_host_ip(self, address: str) -> None:
        try:
            parsed = ipaddress.ip_address(address)
        except ValueError as error:
            raise ValueError("Invalid network address") from error
        if (
            parsed.version != 4
            or parsed.is_loopback
            or parsed.is_link_local
            or parsed.is_multicast
            or parsed.is_unspecified
        ):
            raise ValueError("Choose a reachable IPv4 LAN address")
        with self.lock:
            self.host_ip = address
            if address not in self.host_candidates:
                self.host_candidates.append(address)

    def is_expired(self) -> bool:
        return self.expiry_minutes > 0 and time.time() >= self.expires_at

    def is_available(self) -> bool:
        return self.active and not self.is_expired()

    def seconds_remaining(self) -> int | None:
        if self.expiry_minutes <= 0:
            return None
        return max(0, int(self.expires_at - time.time()))

    def set_expiry(self, minutes: int) -> None:
        if minutes not in {0, 15, 30, 60, 120}:
            raise ValueError("Unsupported expiry")
        with self.lock:
            self.expiry_minutes = minutes
            self.expires_at = time.time() + minutes * 60 if minutes else 0

    def set_mode(self, mode: str) -> None:
        if mode not in {"send", "receive"}:
            raise ValueError("Unsupported mode")
        with self.lock:
            self.mode = mode

    def stop(self) -> None:
        with self.lock:
            self.active = False
            self.generation += 1

    def start(self) -> None:
        with self.lock:
            self.token = secrets.token_urlsafe(24)
            self.clients.clear()
            self.generation += 1
            self.active = True
            if self.expiry_minutes:
                self.expires_at = time.time() + self.expiry_minutes * 60

    def rotate(self) -> None:
        with self.lock:
            self.token = secrets.token_urlsafe(24)
            self.clients.clear()
            self.generation += 1
            self.active = True
            if self.expiry_minutes:
                self.expires_at = time.time() + self.expiry_minutes * 60

    def generation_is_active(self, generation: int) -> bool:
        with self.lock:
            return self.generation == generation and self.is_available()

    def authorize_guest(self, token: str, required_mode: str | None = None) -> int:
        with self.lock:
            if not secrets.compare_digest(token, self.token):
                raise SessionAccessError(404, "Transfer session not found")
            if not self.is_available():
                raise SessionAccessError(410, "This transfer session has ended")
            if required_mode and self.mode != required_mode:
                action = "Uploads" if required_mode == "receive" else "Downloads"
                raise SessionAccessError(403, f"{action} are not enabled")
            return self.generation

    def add_path(self, source: Path, *, owned_copy: bool = False) -> SharedFile:
        source = source.resolve()
        if not source.is_file():
            raise FileNotFoundError(source)
        with self.lock:
            name = self._unique_public_name(safe_filename(source.name))
            item = SharedFile(
                id=uuid.uuid4().hex,
                name=name,
                path=source,
                size=source.stat().st_size,
                mime=mimetypes.guess_type(source.name)[0] or "application/octet-stream",
                owned_copy=owned_copy,
            )
            self.files[item.id] = item
        return item

    def _unique_public_name(self, name: str) -> str:
        existing = {item.name for item in self.files.values()}
        if name not in existing:
            return name
        stem, suffix = Path(name).stem, Path(name).suffix
        for number in range(2, 10_000):
            candidate = f"{stem} ({number}){suffix}"
            if candidate not in existing:
                return candidate
        raise RuntimeError("Could not allocate a unique public filename")

    def reserve_share_copy(self, name: str) -> tuple[Path, BinaryIO]:
        return self._reserve_unique_target(self.share_dir, safe_filename(name))

    def reserve_received_file(self, name: str) -> tuple[Path, BinaryIO]:
        return self._reserve_unique_target(self.receive_dir, safe_filename(name))

    @staticmethod
    def _reserve_unique_target(directory: Path, name: str) -> tuple[Path, BinaryIO]:
        stem, suffix = Path(name).stem, Path(name).suffix
        for number in range(1, 10_000):
            candidate = directory / (name if number == 1 else f"{stem} ({number}){suffix}")
            try:
                descriptor = os.open(candidate, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
                return candidate, os.fdopen(descriptor, "wb")
            except FileExistsError:
                continue
        raise RuntimeError("Could not allocate a unique filename")

    def acquire_file(self, file_id: str) -> tuple[SharedFile, BinaryIO] | None:
        with self.lock:
            item = self.files.get(file_id)
            if not item:
                return None
            try:
                stream = item.path.open("rb")
            except FileNotFoundError:
                return None
            item.readers += 1
            return item, stream

    def release_file(self, item: SharedFile, stream: BinaryIO) -> None:
        stream.close()
        with self.lock:
            item.readers -= 1
            should_delete = item.pending_delete and item.readers == 0 and item.owned_copy
        if should_delete:
            item.path.unlink(missing_ok=True)

    def remove_file(self, file_id: str) -> bool:
        with self.lock:
            item = self.files.pop(file_id, None)
            if item and item.owned_copy and item.readers:
                item.pending_delete = True
                return True
        if item and item.owned_copy:
            item.path.unlink(missing_ok=True)
        return item is not None

    def clear_files(self) -> None:
        for file_id in list(self.files):
            self.remove_file(file_id)

    def note_client(self, address: str) -> None:
        if address:
            with self.lock:
                self.clients[address] = time.time()

    def add_activity(self, *, kind: str, name: str, size: int, device: str) -> None:
        with self.lock:
            self.activities.insert(
                0,
                Activity(
                    id=uuid.uuid4().hex,
                    kind=kind,
                    name=name,
                    size=size,
                    device=device,
                    status="completed",
                    timestamp=time.time(),
                ),
            )
            del self.activities[30:]

    def connected_devices(self) -> int:
        cutoff = time.time() - 20
        with self.lock:
            self.clients = {ip: seen for ip, seen in self.clients.items() if seen >= cutoff}
            return len(self.clients)

    def public_state(self, *, admin: bool) -> dict[str, object]:
        with self.lock:
            state: dict[str, object] = {
                "active": self.is_available(),
                "mode": self.mode,
                "deviceName": self.device_name,
                "files": [item.public() for item in self.files.values()],
                "secondsRemaining": self.seconds_remaining(),
                "expiryMinutes": self.expiry_minutes,
                "connectedDevices": self.connected_devices(),
                "activities": [item.public() for item in self.activities],
                "canDownload": self.mode == "send" and self.is_available(),
                "canUpload": self.mode == "receive" and self.is_available(),
                "features": {
                    "receive": True,
                    "downloadAll": True,
                    "pathEntry": True,
                    "activity": True,
                    "connectedDevices": True,
                },
            }
            if admin:
                state.update(
                    {
                        "shareUrl": self.share_url,
                        "hostIp": self.host_ip,
                        "hostCandidates": self.host_candidates,
                        "receiveDir": str(self.receive_dir),
                    }
                )
            return state

    def close(self) -> None:
        self._temp.cleanup()


def copy_stream(
    source: BinaryIO,
    target: BinaryIO,
    *,
    max_bytes: int,
    still_allowed: Callable[[], bool] | None = None,
) -> int:
    written = 0
    try:
        while chunk := source.read(1024 * 1024):
            if still_allowed and not still_allowed():
                raise TransferRevoked("Transfer session ended during upload")
            written += len(chunk)
            if written > max_bytes:
                raise ValueError("File exceeds the configured size limit")
            target.write(chunk)
    finally:
        target.close()
    return written
