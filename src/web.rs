use async_stream::stream;
use axum::body::Body;
use axum::extract::{ConnectInfo, DefaultBodyLimit, Multipart, Path, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use bytes::Bytes;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use qrcode::QrCode;
use qrcode::render::svg;
use rust_embed::RustEmbed;
use serde::Deserialize;
use serde_json::{Value, json};
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use subtle::ConstantTimeEq;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use crate::network;
use crate::range::{ByteRange, parse_range};
use crate::state::{AccessError, AppState, FileEntry, PublicFile};

const MAX_BROWSER_UPLOAD: u64 = 2 * 1024 * 1024 * 1024;
const CHUNK_SIZE: usize = 128 * 1024;

#[derive(RustEmbed)]
#[folder = "wiferry/static/"]
struct Assets;

#[derive(Deserialize)]
struct PathsRequest {
    paths: Vec<String>,
}

#[derive(Deserialize)]
struct ExpiryRequest {
    minutes: u64,
}

#[derive(Deserialize)]
struct HostRequest {
    address: String,
}

pub fn admin_router(state: Arc<AppState>) -> Router {
    let admin_api = Router::new()
        .route("/api/admin/state", get(admin_state))
        .route("/api/admin/qr", get(admin_qr))
        .route("/api/admin/paths", post(admin_paths))
        .route("/api/admin/files", post(admin_upload).delete(admin_clear))
        .route("/api/admin/files/{id}", delete(admin_remove))
        .route("/api/admin/expiry", post(admin_expiry))
        .route("/api/admin/host-ip", post(admin_host))
        .route("/api/admin/stop", post(admin_stop))
        .route("/api/admin/start", post(admin_start))
        .route("/api/admin/rotate", post(admin_start))
        .layer(DefaultBodyLimit::max(MAX_BROWSER_UPLOAD as usize))
        .route_layer(middleware::from_fn_with_state(state.clone(), admin_guard));

    Router::new()
        .route("/", get(admin_index))
        .route("/assets/{*path}", get(asset))
        .merge(admin_api)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            admin_boundary,
        ))
        .layer(middleware::from_fn(security_headers))
        .with_state(state)
}

pub fn guest_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/s/{token}/", get(guest_index))
        .route("/api/session/{token}", get(guest_state))
        .route(
            "/api/session/{token}/files/{id}",
            get(download).head(download),
        )
        .route("/assets/{*path}", get(asset))
        .layer(middleware::from_fn(security_headers))
        .with_state(state)
}

async fn admin_boundary(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    if !peer.ip().is_loopback() {
        return error(
            StatusCode::FORBIDDEN,
            "The management interface is local-only",
        );
    }
    if !valid_admin_host(request.headers(), state.admin_port) {
        return error(StatusCode::FORBIDDEN, "Invalid management host");
    }
    if !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) && request
        .headers()
        .get(header::ORIGIN)
        .is_some_and(|origin| !valid_admin_origin(origin, state.admin_port))
    {
        return error(StatusCode::FORBIDDEN, "Invalid management origin");
    }
    next.run(request).await
}

async fn admin_guard(State(state): State<Arc<AppState>>, request: Request, next: Next) -> Response {
    let provided = request
        .headers()
        .get("x-wiferry-admin")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let expected = state.admin_token();
    if expected.as_bytes().ct_eq(provided.as_bytes()).unwrap_u8() != 1 {
        return error(StatusCode::FORBIDDEN, "Invalid management token");
    }
    next.run(request).await
}

fn valid_admin_host(headers: &HeaderMap, port: u16) -> bool {
    let Some(host) = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    host.eq_ignore_ascii_case(&format!("127.0.0.1:{port}"))
        || host.eq_ignore_ascii_case(&format!("localhost:{port}"))
}

fn valid_admin_origin(origin: &HeaderValue, port: u16) -> bool {
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    origin.eq_ignore_ascii_case(&format!("http://127.0.0.1:{port}"))
        || origin.eq_ignore_ascii_case(&format!("http://localhost:{port}"))
}

async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers
        .entry(header::CACHE_CONTROL)
        .or_insert(HeaderValue::from_static("no-store"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; img-src 'self' data: blob:; style-src 'self'; script-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'none'",
        ),
    );
    response
}

async fn admin_index() -> Response {
    index(String::new())
}

async fn guest_index(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Path(_token): Path<String>,
) -> Response {
    if !network::guest_allowed(peer, &state.networks) {
        return error(StatusCode::FORBIDDEN, "Client is outside the local network");
    }
    index(String::new())
}

fn index(admin_token: String) -> Response {
    let Some(asset) = Assets::get("index.html") else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "Embedded UI is missing");
    };
    let html = String::from_utf8_lossy(asset.data.as_ref())
        .replace("__WIFERRY_ADMIN_TOKEN__", &admin_token);
    ([(header::CACHE_CONTROL, "no-store")], Html(html)).into_response()
}

async fn asset(Path(path): Path<String>) -> Response {
    let full = format!("assets/{path}");
    match Assets::get(&full) {
        Some(asset) => (
            [
                (header::CONTENT_TYPE, asset.metadata.mimetype()),
                (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
            ],
            asset.data,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn admin_state(State(state): State<Arc<AppState>>) -> Json<crate::state::PublicState> {
    Json(state.public_state(true))
}

async fn admin_qr(State(state): State<Arc<AppState>>) -> Response {
    match QrCode::new(state.share_url().as_bytes()) {
        Ok(code) => {
            let image = code
                .render::<svg::Color>()
                .min_dimensions(320, 320)
                .dark_color(svg::Color("#101827"))
                .light_color(svg::Color("#ffffff"))
                .build();
            (
                [
                    (header::CONTENT_TYPE, "image/svg+xml; charset=utf-8"),
                    (header::CACHE_CONTROL, "no-store"),
                ],
                image,
            )
                .into_response()
        }
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not create QR code",
        ),
    }
}

async fn admin_paths(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PathsRequest>,
) -> Response {
    let mut added = Vec::new();
    for raw in payload.paths {
        match state.add_path(PathBuf::from(&raw).as_path(), false) {
            Ok(item) => added.push(item),
            Err(detail) => return error(StatusCode::BAD_REQUEST, &detail),
        }
    }
    Json(json!({"files": added})).into_response()
}

async fn admin_upload(State(state): State<Arc<AppState>>, mut multipart: Multipart) -> Response {
    let Ok(_permit) = state.upload_slots.acquire().await else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "Upload service stopped");
    };
    let mut added: Vec<PublicFile> = Vec::new();
    loop {
        let mut field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(_) => return error(StatusCode::BAD_REQUEST, "Malformed multipart upload"),
        };
        let Some(filename) = field.file_name().map(ToOwned::to_owned) else {
            continue;
        };
        let mut target = match state.temp_target(&filename) {
            Ok(path) => path,
            Err(detail) => return error(StatusCode::INSUFFICIENT_STORAGE, &detail),
        };
        let mut output = loop {
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .await
            {
                Ok(file) => break file,
                Err(open_error) if open_error.kind() == io::ErrorKind::AlreadyExists => {
                    target = match state.temp_target(&filename) {
                        Ok(path) => path,
                        Err(detail) => return error(StatusCode::INSUFFICIENT_STORAGE, &detail),
                    };
                }
                Err(_) => {
                    return error(
                        StatusCode::INSUFFICIENT_STORAGE,
                        "Could not create temporary file",
                    );
                }
            }
        };
        let mut written = 0_u64;
        loop {
            let chunk = match field.chunk().await {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break,
                Err(_) => {
                    drop(output);
                    let _ = tokio::fs::remove_file(&target).await;
                    return error(StatusCode::BAD_REQUEST, "Upload stream was interrupted");
                }
            };
            written += chunk.len() as u64;
            if written > MAX_BROWSER_UPLOAD {
                drop(output);
                let _ = tokio::fs::remove_file(&target).await;
                return error(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "Browser uploads are limited to 2 GB",
                );
            }
            if output.write_all(&chunk).await.is_err() {
                drop(output);
                let _ = tokio::fs::remove_file(&target).await;
                return error(
                    StatusCode::INSUFFICIENT_STORAGE,
                    "Could not save temporary file",
                );
            }
        }
        if output.flush().await.is_err() {
            let _ = tokio::fs::remove_file(&target).await;
            return error(
                StatusCode::INSUFFICIENT_STORAGE,
                "Could not finish temporary file",
            );
        }
        drop(output);
        match state.add_path(&target, true) {
            Ok(item) => added.push(item),
            Err(detail) => {
                let _ = tokio::fs::remove_file(&target).await;
                return error(StatusCode::BAD_REQUEST, &detail);
            }
        }
    }
    Json(json!({"files": added})).into_response()
}

async fn admin_remove(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    if state.remove(&id) {
        Json(json!({"ok": true})).into_response()
    } else {
        error(StatusCode::NOT_FOUND, "File not found")
    }
}

async fn admin_clear(State(state): State<Arc<AppState>>) -> Json<Value> {
    state.clear();
    Json(json!({"ok": true}))
}

async fn admin_expiry(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ExpiryRequest>,
) -> Response {
    match state.set_expiry(payload.minutes) {
        Ok(()) => Json(state.public_state(true)).into_response(),
        Err(detail) => error(StatusCode::BAD_REQUEST, &detail),
    }
}

async fn admin_host(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<HostRequest>,
) -> Response {
    let Ok(address) = payload.address.parse::<Ipv4Addr>() else {
        return error(StatusCode::BAD_REQUEST, "Invalid IPv4 address");
    };
    match state.set_host_ip(address) {
        Ok(()) => Json(state.public_state(true)).into_response(),
        Err(detail) => error(StatusCode::BAD_REQUEST, &detail),
    }
}

async fn admin_stop(State(state): State<Arc<AppState>>) -> Json<crate::state::PublicState> {
    state.stop();
    Json(state.public_state(true))
}

async fn admin_start(State(state): State<Arc<AppState>>) -> Json<crate::state::PublicState> {
    state.start_or_rotate();
    Json(state.public_state(true))
}

async fn guest_state(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Path(token): Path<String>,
) -> Response {
    if !network::guest_allowed(peer, &state.networks) {
        return error(StatusCode::FORBIDDEN, "Client is outside the local network");
    }
    match state.authorize(&token) {
        Ok(_) => Json(state.public_state(false)).into_response(),
        Err(reason) => access_error(reason),
    }
}

async fn download(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Path((token, id)): Path<(String, String)>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    if !network::guest_allowed(peer, &state.networks) {
        return error(StatusCode::FORBIDDEN, "Client is outside the local network");
    }
    let authorization = match state.authorize(&token) {
        Ok(value) => value,
        Err(reason) => return access_error(reason),
    };
    let Some(item) = state.file(&id) else {
        return error(StatusCode::NOT_FOUND, "File not found");
    };
    match tokio::fs::metadata(&item.path).await {
        Ok(value)
            if value.len() == item.size
                && value
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|duration| duration.as_nanos())
                    .unwrap_or(0)
                    == item.modified_nanos => {}
        _ => {
            return error(
                StatusCode::CONFLICT,
                "Source file changed after it was shared",
            );
        }
    }
    let requested = match parse_range(&headers, item.size) {
        Ok(value) => value,
        Err(status) => {
            let mut response = status.into_response();
            response
                .headers_mut()
                .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
            response.headers_mut().insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes */{}", item.size)).unwrap(),
            );
            return response;
        }
    };
    if item.size == 0 {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::OK;
        add_download_headers(response.headers_mut(), &item, 0);
        return response;
    }
    let range = requested.unwrap_or(ByteRange {
        start: 0,
        end: item.size.saturating_sub(1),
    });
    let mut file = match File::open(&item.path).await {
        Ok(value) => value,
        Err(_) => return error(StatusCode::NOT_FOUND, "File is no longer available"),
    };
    if file.seek(io::SeekFrom::Start(range.start)).await.is_err() {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not seek source file",
        );
    }
    let state_for_stream = state.clone();
    let token_for_stream = token.clone();
    let generation = authorization.generation;
    let length = range.len();
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        let stream = stream! {
            let mut remaining = length;
            let mut buffer = vec![0_u8; CHUNK_SIZE];
            while remaining > 0 {
                if !state_for_stream.authorization_active(&token_for_stream, generation) {
                    yield Err::<Bytes, io::Error>(io::Error::new(io::ErrorKind::ConnectionAborted, "share revoked"));
                    break;
                }
                let wanted = remaining.min(CHUNK_SIZE as u64) as usize;
                let read = match file.read(&mut buffer[..wanted]).await {
                    Ok(value) => value,
                    Err(read_error) => {
                        yield Err::<Bytes, io::Error>(read_error);
                        break;
                    }
                };
                if read == 0 {
                    yield Err::<Bytes, io::Error>(io::Error::new(io::ErrorKind::UnexpectedEof, "source file changed"));
                    break;
                }
                remaining -= read as u64;
                yield Ok::<Bytes, io::Error>(Bytes::copy_from_slice(&buffer[..read]));
            }
        };
        Body::from_stream(stream)
    };
    let mut response = Response::new(body);
    *response.status_mut() = if requested.is_some() {
        StatusCode::PARTIAL_CONTENT
    } else {
        StatusCode::OK
    };
    let response_headers = response.headers_mut();
    add_download_headers(response_headers, &item, length);
    if requested.is_some() {
        response_headers.insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&format!(
                "bytes {}-{}/{}",
                range.start, range.end, item.size
            ))
            .unwrap(),
        );
    }
    response
}

fn add_download_headers(headers: &mut HeaderMap, item: &FileEntry, length: u64) {
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&length.to_string()).unwrap(),
    );
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&item.mime).unwrap(),
    );
    let encoded = utf8_percent_encode(&item.name, NON_ALPHANUMERIC).to_string();
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename*=UTF-8''{encoded}")).unwrap(),
    );
    headers.insert(
        header::ETAG,
        HeaderValue::from_str(&format!(
            "W/\"{}-{}-{}\"",
            item.id, item.size, item.modified_nanos
        ))
        .unwrap(),
    );
}

fn access_error(reason: AccessError) -> Response {
    match reason {
        AccessError::NotFound => error(StatusCode::NOT_FOUND, "Transfer session not found"),
        AccessError::Ended => error(StatusCode::GONE, "This transfer session has ended"),
    }
}

fn error(status: StatusCode, detail: &str) -> Response {
    (status, Json(json!({"detail": detail}))).into_response()
}
