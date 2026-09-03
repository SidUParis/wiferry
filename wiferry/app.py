from __future__ import annotations

import asyncio
import io
import json
import re
import tempfile
import zipfile
from collections.abc import Iterator
from pathlib import Path
from typing import BinaryIO

import qrcode
from fastapi import FastAPI, File, Header, HTTPException, Request, UploadFile
from fastapi.responses import HTMLResponse, Response, StreamingResponse
from fastapi.staticfiles import StaticFiles
from pydantic import BaseModel

from .core import (
    SessionAccessError,
    SharedFile,
    TransferRevoked,
    TransferSession,
    copy_stream,
)

STATIC_DIR = Path(__file__).parent / "static"
MAX_FILE_BYTES = 2 * 1024 * 1024 * 1024
MAX_REQUEST_BYTES = MAX_FILE_BYTES
MAX_ZIP_SOURCE_BYTES = 2 * 1024 * 1024 * 1024
MAX_CONCURRENT_UPLOADS = 2
LOOPBACKS = {"127.0.0.1", "::1", "localhost", "testclient"}


class PayloadTooLarge(Exception):
    pass


class UploadGuardMiddleware:
    """Authorize and bound multipart requests before FastAPI parses their body."""

    def __init__(self, app, *, session: TransferSession, allow_non_loopback_admin: bool):
        self.app = app
        self.session = session
        self.allow_non_loopback_admin = allow_non_loopback_admin
        self.active_uploads = 0
        self.slot_lock = asyncio.Lock()

    async def _reject(self, send, status: int, detail: str) -> None:
        body = json.dumps({"detail": detail}).encode("utf-8")
        await send(
            {
                "type": "http.response.start",
                "status": status,
                "headers": [
                    (b"content-type", b"application/json"),
                    (b"content-length", str(len(body)).encode()),
                    (b"cache-control", b"no-store"),
                ],
            }
        )
        await send({"type": "http.response.body", "body": body})

    async def __call__(self, scope, receive, send) -> None:
        if scope["type"] != "http" or scope["method"] != "POST":
            await self.app(scope, receive, send)
            return
        path = scope.get("path", "")
        guest_match = re.fullmatch(r"/api/session/([^/]+)/upload", path)
        is_admin_upload = path == "/api/admin/files"
        if not guest_match and not is_admin_upload:
            await self.app(scope, receive, send)
            return

        headers = {key.lower(): value for key, value in scope.get("headers", [])}
        if is_admin_upload:
            address = scope.get("client", ("", 0))[0]
            is_loopback = address in LOOPBACKS or address.startswith("::ffff:127.")
            provided = headers.get(b"x-wiferry-admin", b"").decode("utf-8", "ignore")
            if not self.allow_non_loopback_admin and not is_loopback:
                await self._reject(send, 403, "The management interface is local-only")
                return
            if not provided or not secrets_compare(provided, self.session.admin_token):
                await self._reject(send, 403, "Invalid management token")
                return
        guest_generation = None
        if not is_admin_upload:
            token = guest_match.group(1)
            try:
                guest_generation = self.session.authorize_guest(token, "receive")
            except SessionAccessError as error:
                await self._reject(send, error.status, error.detail)
                return

        content_length = headers.get(b"content-length")
        if content_length:
            try:
                if int(content_length) > MAX_REQUEST_BYTES:
                    await self._reject(send, 413, "Upload exceeds the 2 GB request limit")
                    return
            except ValueError:
                await self._reject(send, 400, "Invalid Content-Length")
                return

        async with self.slot_lock:
            if self.active_uploads >= MAX_CONCURRENT_UPLOADS:
                await self._reject(send, 429, "Too many uploads in progress")
                return
            self.active_uploads += 1

        received = 0
        response_started = False

        async def limited_receive():
            nonlocal received
            if guest_generation is not None and not self.session.generation_is_active(
                guest_generation
            ):
                raise TransferRevoked
            message = await receive()
            if message["type"] == "http.request":
                received += len(message.get("body", b""))
                if received > MAX_REQUEST_BYTES:
                    raise PayloadTooLarge
            return message

        async def tracked_send(message):
            nonlocal response_started
            if message["type"] == "http.response.start":
                response_started = True
            await send(message)

        try:
            await self.app(scope, limited_receive, tracked_send)
        except PayloadTooLarge:
            if not response_started:
                await self._reject(send, 413, "Upload exceeds the 2 GB request limit")
        except TransferRevoked:
            if not response_started:
                await self._reject(send, 410, "This transfer session has ended")
        finally:
            async with self.slot_lock:
                self.active_uploads -= 1


class ModeRequest(BaseModel):
    mode: str


class ExpiryRequest(BaseModel):
    minutes: int


class HostAddressRequest(BaseModel):
    address: str


class PathsRequest(BaseModel):
    paths: list[str]


def _index_html(admin_token: str = "") -> str:
    index_path = STATIC_DIR / "index.html"
    if not index_path.is_file():
        raise HTTPException(503, "Frontend has not been built")
    return index_path.read_text(encoding="utf-8").replace("__WIFERRY_ADMIN_TOKEN__", admin_token)


def _assert_loopback(request: Request, allow_non_loopback: bool) -> None:
    address = request.client.host if request.client else ""
    if not allow_non_loopback and address not in LOOPBACKS:
        raise HTTPException(403, "The management interface is local-only")


def _assert_admin(request: Request, session: TransferSession, provided: str | None) -> None:
    _assert_loopback(request, request.app.state.allow_non_loopback_admin)
    if not provided or not secrets_compare(provided, session.admin_token):
        raise HTTPException(403, "Invalid management token")


def secrets_compare(left: str, right: str) -> bool:
    import secrets

    return secrets.compare_digest(left, right)


def _authorize_guest(session: TransferSession, token: str, required_mode: str | None = None) -> int:
    try:
        return session.authorize_guest(token, required_mode)
    except SessionAccessError as error:
        raise HTTPException(error.status, error.detail) from error


def _iter_range(
    stream: BinaryIO,
    item: SharedFile,
    session: TransferSession,
    generation: int,
    start: int,
    length: int,
    on_complete,
) -> Iterator[bytes]:
    complete = False
    try:
        stream.seek(start)
        remaining = length
        while remaining:
            if not session.generation_is_active(generation):
                raise TransferRevoked("Transfer session ended during download")
            chunk = stream.read(min(1024 * 1024, remaining))
            if not chunk:
                break
            remaining -= len(chunk)
            yield chunk
        complete = remaining == 0
    finally:
        session.release_file(item, stream)
        if complete and on_complete:
            on_complete()


def _iter_temp_archive(path: Path, session: TransferSession, generation: int) -> Iterator[bytes]:
    try:
        with path.open("rb") as stream:
            while chunk := stream.read(1024 * 1024):
                if not session.generation_is_active(generation):
                    raise TransferRevoked("Transfer session ended during ZIP download")
                yield chunk
    finally:
        path.unlink(missing_ok=True)


def ranged_file_response(
    item: SharedFile,
    stream: BinaryIO,
    request: Request,
    session: TransferSession,
    generation: int,
    on_complete,
) -> Response:
    size = item.size
    start, end, status = 0, size - 1, 200
    range_header = request.headers.get("range", "")
    if range_header:
        match = re.fullmatch(r"bytes=(\d*)-(\d*)", range_header.strip())
        if not match:
            session.release_file(item, stream)
            return Response(status_code=416, headers={"Content-Range": f"bytes */{size}"})
        first, last = match.groups()
        try:
            if first:
                start = int(first)
                end = int(last) if last else end
            elif last:
                start = max(0, size - int(last))
            else:
                raise ValueError
            end = min(end, size - 1)
            if start < 0 or start > end:
                raise ValueError
        except ValueError:
            session.release_file(item, stream)
            return Response(status_code=416, headers={"Content-Range": f"bytes */{size}"})
        status = 206

    length = end - start + 1
    headers = {
        "Accept-Ranges": "bytes",
        "Content-Length": str(length),
        "Content-Disposition": f"attachment; filename*=UTF-8''{quote_header(item.name)}",
        "Cache-Control": "private, no-store",
    }
    if status == 206:
        headers["Content-Range"] = f"bytes {start}-{end}/{size}"
    return StreamingResponse(
        _iter_range(stream, item, session, generation, start, length, on_complete),
        status_code=status,
        media_type=item.mime,
        headers=headers,
    )


def quote_header(value: str) -> str:
    from urllib.parse import quote

    return quote(value, safe="")


def create_app(session: TransferSession, *, allow_non_loopback_admin: bool = False) -> FastAPI:
    app = FastAPI(title="Wiferry", docs_url=None, redoc_url=None, openapi_url=None)
    app.state.session = session
    app.state.allow_non_loopback_admin = allow_non_loopback_admin

    @app.middleware("http")
    async def security_headers(request: Request, call_next):
        response = await call_next(request)
        response.headers["X-Content-Type-Options"] = "nosniff"
        response.headers["Referrer-Policy"] = "no-referrer"
        response.headers["X-Frame-Options"] = "DENY"
        response.headers["Permissions-Policy"] = "camera=(), microphone=(), geolocation=()"
        response.headers["Content-Security-Policy"] = (
            "default-src 'self'; img-src 'self' data: blob:; style-src 'self'; "
            "script-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'none'"
        )
        return response

    @app.get("/", response_class=HTMLResponse)
    def admin_index(request: Request) -> str:
        _assert_loopback(request, allow_non_loopback_admin)
        return _index_html(session.admin_token)

    @app.get("/s/{token}/", response_class=HTMLResponse)
    def guest_index(token: str, request: Request) -> str:
        try:
            session.authorize_guest(token)
            session.note_client(request.client.host if request.client else "unknown")
        except SessionAccessError:
            pass
        return _index_html()

    @app.get("/api/admin/state")
    def admin_state(request: Request, x_wiferry_admin: str | None = Header(default=None)):
        _assert_admin(request, session, x_wiferry_admin)
        return session.public_state(admin=True)

    @app.get("/api/admin/qr")
    def admin_qr(request: Request, x_wiferry_admin: str | None = Header(default=None)):
        _assert_admin(request, session, x_wiferry_admin)
        qr = qrcode.QRCode(version=None, box_size=8, border=2)
        qr.add_data(session.share_url)
        qr.make(fit=True)
        image = qr.make_image(fill_color="#101827", back_color="#ffffff")
        buffer = io.BytesIO()
        image.save(buffer, format="PNG")
        return Response(
            buffer.getvalue(), media_type="image/png", headers={"Cache-Control": "no-store"}
        )

    @app.post("/api/admin/files")
    def admin_add_files(
        request: Request,
        files: list[UploadFile] = File(...),
        x_wiferry_admin: str | None = Header(default=None),
    ):
        _assert_admin(request, session, x_wiferry_admin)
        added = []
        added_ids: list[str] = []
        current_target: Path | None = None
        try:
            for upload in files:
                current_target, target_stream = session.reserve_share_copy(
                    upload.filename or "shared-file"
                )
                copy_stream(upload.file, target_stream, max_bytes=MAX_FILE_BYTES)
                item = session.add_path(current_target, owned_copy=True)
                added_ids.append(item.id)
                added.append(item.public())
                current_target = None
        except ValueError as error:
            if current_target:
                current_target.unlink(missing_ok=True)
            for file_id in added_ids:
                session.remove_file(file_id)
            raise HTTPException(413, str(error)) from error
        except OSError as error:
            if current_target:
                current_target.unlink(missing_ok=True)
            for file_id in added_ids:
                session.remove_file(file_id)
            raise HTTPException(507, "Could not store uploaded files") from error
        finally:
            for upload in files:
                upload.file.close()
        return {"files": added}

    @app.delete("/api/admin/files/{file_id}")
    def admin_remove_file(
        file_id: str,
        request: Request,
        x_wiferry_admin: str | None = Header(default=None),
    ):
        _assert_admin(request, session, x_wiferry_admin)
        if not session.remove_file(file_id):
            raise HTTPException(404, "File not found")
        return {"ok": True}

    @app.delete("/api/admin/files")
    def admin_clear_files(request: Request, x_wiferry_admin: str | None = Header(default=None)):
        _assert_admin(request, session, x_wiferry_admin)
        session.clear_files()
        return {"ok": True}

    @app.post("/api/admin/paths")
    def admin_add_paths(
        payload: PathsRequest,
        request: Request,
        x_wiferry_admin: str | None = Header(default=None),
    ):
        _assert_admin(request, session, x_wiferry_admin)
        added = []
        added_ids: list[str] = []
        try:
            for raw_path in payload.paths:
                path = Path(raw_path).expanduser()
                if not path.is_file():
                    raise ValueError(f"File does not exist: {raw_path}")
                item = session.add_path(path)
                added_ids.append(item.id)
                added.append(item.public())
        except (OSError, ValueError) as error:
            for file_id in added_ids:
                session.remove_file(file_id)
            raise HTTPException(400, str(error)) from error
        return {"files": added}

    @app.post("/api/admin/mode")
    def admin_mode(
        payload: ModeRequest,
        request: Request,
        x_wiferry_admin: str | None = Header(default=None),
    ):
        _assert_admin(request, session, x_wiferry_admin)
        try:
            session.set_mode(payload.mode)
        except ValueError as error:
            raise HTTPException(400, str(error)) from error
        return session.public_state(admin=True)

    @app.post("/api/admin/expiry")
    def admin_expiry(
        payload: ExpiryRequest,
        request: Request,
        x_wiferry_admin: str | None = Header(default=None),
    ):
        _assert_admin(request, session, x_wiferry_admin)
        try:
            session.set_expiry(payload.minutes)
        except ValueError as error:
            raise HTTPException(400, str(error)) from error
        return session.public_state(admin=True)

    @app.post("/api/admin/host-ip")
    def admin_host_ip(
        payload: HostAddressRequest,
        request: Request,
        x_wiferry_admin: str | None = Header(default=None),
    ):
        _assert_admin(request, session, x_wiferry_admin)
        try:
            session.set_host_ip(payload.address)
        except ValueError as error:
            raise HTTPException(400, str(error)) from error
        session.rotate()
        return session.public_state(admin=True)

    @app.post("/api/admin/stop")
    def admin_stop(request: Request, x_wiferry_admin: str | None = Header(default=None)):
        _assert_admin(request, session, x_wiferry_admin)
        session.stop()
        return session.public_state(admin=True)

    @app.post("/api/admin/start")
    def admin_start(request: Request, x_wiferry_admin: str | None = Header(default=None)):
        _assert_admin(request, session, x_wiferry_admin)
        session.start()
        return session.public_state(admin=True)

    @app.post("/api/admin/rotate")
    def admin_rotate(request: Request, x_wiferry_admin: str | None = Header(default=None)):
        _assert_admin(request, session, x_wiferry_admin)
        session.rotate()
        return session.public_state(admin=True)

    @app.get("/api/session/{token}")
    def guest_state(token: str, request: Request):
        _authorize_guest(session, token)
        session.note_client(request.client.host if request.client else "unknown")
        return session.public_state(admin=False)

    @app.get("/api/session/{token}/files/{file_id}")
    def guest_download(token: str, file_id: str, request: Request):
        generation = _authorize_guest(session, token, "send")
        acquired = session.acquire_file(file_id)
        if not acquired:
            raise HTTPException(404, "File not found")
        item, stream = acquired
        device = request.client.host if request.client else "nearby device"
        session.note_client(device)
        range_header = request.headers.get("range")
        on_complete = None
        if not range_header:
            on_complete = lambda: session.add_activity(
                kind="download", name=item.name, size=item.size, device=device
            )
        return ranged_file_response(item, stream, request, session, generation, on_complete)

    @app.get("/api/session/{token}/download-all")
    def guest_download_all(token: str, request: Request):
        generation = _authorize_guest(session, token, "send")
        if not session.files:
            raise HTTPException(403, "Downloads are not available")
        with session.lock:
            file_ids = list(session.files)
            total_size = sum(item.size for item in session.files.values())
        if total_size > MAX_ZIP_SOURCE_BYTES:
            raise HTTPException(413, "Download all is limited to 2 GB")
        with tempfile.NamedTemporaryFile(prefix="wiferry-", suffix=".zip", delete=False) as archive:
            archive_path = Path(archive.name)
        try:
            with zipfile.ZipFile(archive_path, "w", compression=zipfile.ZIP_STORED) as bundle:
                for file_id in file_ids:
                    acquired = session.acquire_file(file_id)
                    if not acquired:
                        raise FileNotFoundError(file_id)
                    item, source = acquired
                    try:
                        with bundle.open(item.name, "w") as target:
                            while chunk := source.read(1024 * 1024):
                                if not session.generation_is_active(generation):
                                    raise TransferRevoked
                                target.write(chunk)
                    finally:
                        session.release_file(item, source)
        except TransferRevoked as error:
            archive_path.unlink(missing_ok=True)
            raise HTTPException(410, "This transfer session has ended") from error
        except OSError as error:
            archive_path.unlink(missing_ok=True)
            raise HTTPException(507, "Could not prepare the ZIP archive") from error
        size = archive_path.stat().st_size
        session.add_activity(
            kind="download",
            name="Wiferry files.zip",
            size=size,
            device=request.client.host if request.client else "nearby device",
        )
        return StreamingResponse(
            _iter_temp_archive(archive_path, session, generation),
            media_type="application/zip",
            headers={
                "Content-Length": str(size),
                "Content-Disposition": 'attachment; filename="Wiferry files.zip"',
                "Cache-Control": "private, no-store",
            },
        )

    @app.post("/api/session/{token}/upload")
    def guest_upload(token: str, request: Request, files: list[UploadFile] = File(...)):
        generation = _authorize_guest(session, token, "receive")
        saved = []
        saved_paths: list[Path] = []
        current_target: Path | None = None
        try:
            for upload in files:
                current_target, target_stream = session.reserve_received_file(
                    upload.filename or "received-file"
                )
                size = copy_stream(
                    upload.file,
                    target_stream,
                    max_bytes=MAX_FILE_BYTES,
                    still_allowed=lambda: session.generation_is_active(generation),
                )
                saved_paths.append(current_target)
                saved.append({"name": current_target.name, "size": size})
                current_target = None
        except TransferRevoked as error:
            if current_target:
                current_target.unlink(missing_ok=True)
            for path in saved_paths:
                path.unlink(missing_ok=True)
            raise HTTPException(410, str(error)) from error
        except ValueError as error:
            if current_target:
                current_target.unlink(missing_ok=True)
            for path in saved_paths:
                path.unlink(missing_ok=True)
            raise HTTPException(413, str(error)) from error
        except OSError as error:
            if current_target:
                current_target.unlink(missing_ok=True)
            for path in saved_paths:
                path.unlink(missing_ok=True)
            raise HTTPException(507, "Could not save uploaded files") from error
        finally:
            for upload in files:
                upload.file.close()
        device = request.client.host if request.client else "nearby device"
        for item in saved:
            session.add_activity(kind="upload", name=item["name"], size=item["size"], device=device)
        return {"files": saved}

    if (STATIC_DIR / "assets").is_dir():
        app.mount("/assets", StaticFiles(directory=STATIC_DIR / "assets"), name="assets")

    app.add_middleware(
        UploadGuardMiddleware,
        session=session,
        allow_non_loopback_admin=allow_non_loopback_admin,
    )

    return app
