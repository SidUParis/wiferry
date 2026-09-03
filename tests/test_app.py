from __future__ import annotations

import hashlib
import io
import time
import zipfile
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from wiferry.app import MAX_REQUEST_BYTES, _iter_range, create_app
from wiferry.core import TransferRevoked, TransferSession, safe_filename


@pytest.fixture
def session(tmp_path: Path):
    item = TransferSession(
        host_ip="192.168.1.50",
        port=8765,
        receive_dir=tmp_path / "received",
        device_name="Test Laptop",
    )
    yield item
    item.close()


@pytest.fixture
def client(session: TransferSession):
    with TestClient(create_app(session, allow_non_loopback_admin=True)) as test_client:
        yield test_client


def admin_headers(session: TransferSession) -> dict[str, str]:
    return {"X-Wiferry-Admin": session.admin_token}


def test_safe_filename_removes_path_components() -> None:
    assert safe_filename("../../private.txt") == "private.txt"
    assert safe_filename(r"C:\\Users\\person\\photo.jpg") == "photo.jpg"
    assert safe_filename("..") == "shared-file"
    assert safe_filename("CON.txt") == "_CON.txt"
    assert safe_filename("bad<name>:file?.txt") == "bad_name__file_.txt"
    assert len(safe_filename("😀" * 240 + ".txt").encode("utf-8")) <= 240
    assert len(safe_filename("a." + "😀" * 200).encode("utf-8")) <= 240


def test_default_device_name_drops_domain_suffix(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setattr("wiferry.core.socket.gethostname", lambda: "laptop.example.test")
    item = TransferSession(host_ip="192.168.1.2", port=8765, receive_dir=tmp_path)
    try:
        assert item.device_name == "laptop"
    finally:
        item.close()


def test_admin_requires_capability_token(client: TestClient) -> None:
    assert client.get("/api/admin/state").status_code == 403


def test_frontend_security_policy_allows_generated_qr_blob(client: TestClient) -> None:
    response = client.get("/")
    assert response.status_code == 200
    assert "Wiferry" in response.text
    assert "img-src 'self' data: blob:" in response.headers["content-security-policy"]


def test_share_download_and_range(
    client: TestClient, session: TransferSession, tmp_path: Path
) -> None:
    source = tmp_path / "presentation.mp3"
    content = b"ID3" + bytes(range(256)) * 64
    source.write_bytes(content)
    item = session.add_path(source)

    state = client.get("/api/admin/state", headers=admin_headers(session))
    assert state.status_code == 200
    assert state.json()["files"][0]["name"] == "presentation.mp3"
    assert state.json()["shareUrl"].startswith("http://192.168.1.50:8765/s/")

    public = client.get(f"/api/session/{session.token}")
    assert public.status_code == 200
    assert public.json()["canDownload"] is True
    assert "shareUrl" not in public.json()

    ranged = client.get(
        f"/api/session/{session.token}/files/{item.id}", headers={"Range": "bytes=3-18"}
    )
    assert ranged.status_code == 206
    assert ranged.content == content[3:19]
    assert ranged.headers["content-range"] == f"bytes 3-18/{len(content)}"
    assert ranged.headers["accept-ranges"] == "bytes"

    complete = client.get(f"/api/session/{session.token}/files/{item.id}")
    assert complete.status_code == 200
    assert hashlib.sha256(complete.content).digest() == hashlib.sha256(content).digest()


def test_browser_upload_remove_and_clear(client: TestClient, session: TransferSession) -> None:
    response = client.post(
        "/api/admin/files",
        headers=admin_headers(session),
        files=[("files", ("notes.txt", b"hello nearby", "text/plain"))],
    )
    assert response.status_code == 200
    file_id = response.json()["files"][0]["id"]
    copied_path = session.files[file_id].path
    assert copied_path.read_bytes() == b"hello nearby"

    removed = client.delete(f"/api/admin/files/{file_id}", headers=admin_headers(session))
    assert removed.status_code == 200
    assert not copied_path.exists()


def test_receive_mode_saves_upload(client: TestClient, session: TransferSession) -> None:
    mode = client.post(
        "/api/admin/mode",
        headers=admin_headers(session),
        json={"mode": "receive"},
    )
    assert mode.status_code == 200
    assert mode.json()["canUpload"] is True

    uploaded = client.post(
        f"/api/session/{session.token}/upload",
        files=[("files", ("../camera photo.jpg", b"jpeg-data", "image/jpeg"))],
    )
    assert uploaded.status_code == 200
    saved_name = uploaded.json()["files"][0]["name"]
    assert saved_name == "camera photo.jpg"
    assert (session.receive_dir / saved_name).read_bytes() == b"jpeg-data"

    state = client.get("/api/admin/state", headers=admin_headers(session)).json()
    assert state["activities"][0]["kind"] == "upload"


def test_invalid_token_and_stopped_session(client: TestClient, session: TransferSession) -> None:
    assert client.get("/s/not-the-token/").status_code == 200
    assert client.get("/api/session/not-the-token").status_code == 404
    stopped = client.post("/api/admin/stop", headers=admin_headers(session))
    assert stopped.status_code == 200
    assert stopped.json()["active"] is False
    assert client.get(f"/api/session/{session.token}").status_code == 410


def test_rotate_revokes_old_link(client: TestClient, session: TransferSession) -> None:
    old_token = session.token
    response = client.post("/api/admin/rotate", headers=admin_headers(session))
    assert response.status_code == 200
    assert session.token != old_token
    assert client.get(f"/api/session/{old_token}").status_code == 404
    assert client.get(f"/api/session/{session.token}").status_code == 200


def test_restart_always_issues_a_new_guest_capability(
    client: TestClient, session: TransferSession
) -> None:
    old_token = session.token
    client.post("/api/admin/stop", headers=admin_headers(session))
    restarted = client.post("/api/admin/start", headers=admin_headers(session))
    assert restarted.status_code == 200
    assert session.token != old_token
    assert client.get(f"/api/session/{old_token}").status_code == 404


def test_changing_qr_network_address_rotates_session(
    client: TestClient, session: TransferSession
) -> None:
    old_token = session.token
    response = client.post(
        "/api/admin/host-ip",
        headers=admin_headers(session),
        json={"address": "10.20.30.40"},
    )
    assert response.status_code == 200
    assert response.json()["hostIp"] == "10.20.30.40"
    assert response.json()["shareUrl"].startswith("http://10.20.30.40:8765/")
    assert session.token != old_token
    rejected = client.post(
        "/api/admin/host-ip",
        headers=admin_headers(session),
        json={"address": "127.0.0.1"},
    )
    assert rejected.status_code == 400


def test_duplicate_basenames_get_unique_public_names(
    session: TransferSession, tmp_path: Path
) -> None:
    left = tmp_path / "left"
    right = tmp_path / "right"
    left.mkdir()
    right.mkdir()
    (left / "report.txt").write_text("left", encoding="utf-8")
    (right / "report.txt").write_text("right", encoding="utf-8")
    first = session.add_path(left / "report.txt")
    second = session.add_path(right / "report.txt")
    assert first.name == "report.txt"
    assert second.name == "report (2).txt"


def test_destination_reservation_is_atomic(session: TransferSession) -> None:
    first_path, first_stream = session.reserve_received_file("same.txt")
    second_path, second_stream = session.reserve_received_file("same.txt")
    try:
        assert first_path != second_path
        assert second_path.name == "same (2).txt"
    finally:
        first_stream.close()
        second_stream.close()
        first_path.unlink()
        second_path.unlink()


def test_upload_guard_rejects_before_multipart_parsing(
    client: TestClient, session: TransferSession
) -> None:
    session.set_mode("receive")
    too_large = str(MAX_REQUEST_BYTES + 1)
    response = client.post(
        f"/api/session/{session.token}/upload",
        content=b"not multipart",
        headers={"Content-Length": too_large},
    )
    assert response.status_code == 413
    invalid = client.post(
        "/api/session/wrong-token/upload",
        content=b"not multipart",
        headers={"Content-Length": too_large},
    )
    assert invalid.status_code == 404


def test_active_download_stops_after_revocation(session: TransferSession, tmp_path: Path) -> None:
    source = tmp_path / "large.bin"
    source.write_bytes(b"a" * (2 * 1024 * 1024 + 10))
    item = session.add_path(source)
    acquired = session.acquire_file(item.id)
    assert acquired is not None
    acquired_item, stream = acquired
    generation = session.generation
    iterator = _iter_range(
        stream,
        acquired_item,
        session,
        generation,
        0,
        item.size,
        None,
    )
    assert len(next(iterator)) == 1024 * 1024
    session.stop()
    with pytest.raises(TransferRevoked):
        next(iterator)
    assert acquired_item.readers == 0


def test_download_all_contains_only_shared_basenames(
    client: TestClient, session: TransferSession, tmp_path: Path
) -> None:
    first = tmp_path / "first.txt"
    second = tmp_path / "second.txt"
    first.write_text("one", encoding="utf-8")
    second.write_text("two", encoding="utf-8")
    session.add_path(first)
    session.add_path(second)

    response = client.get(f"/api/session/{session.token}/download-all")
    assert response.status_code == 200
    with zipfile.ZipFile(io.BytesIO(response.content)) as bundle:
        assert bundle.namelist() == ["first.txt", "second.txt"]
        assert bundle.read("first.txt") == b"one"


def test_expired_session_returns_gone(client: TestClient, session: TransferSession) -> None:
    session.expires_at = time.time() - 1
    assert client.get(f"/api/session/{session.token}").status_code == 410
