use reqwest::blocking::Client;
use reqwest::header::{HOST, ORIGIN, RANGE};
use serde_json::Value;
use std::io::{BufRead, BufReader, Read};
use std::net::{Ipv4Addr, TcpListener};
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

struct Server {
    child: Child,
    _stdout: BufReader<ChildStdout>,
    admin_base: String,
    admin_token: String,
    guest_base: String,
    guest_token: String,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn client() -> Client {
    Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap()
}

fn streaming_client() -> Client {
    Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_secs(5))
        .build()
        .unwrap()
}

fn free_port() -> u16 {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn spawn_server(sources: &[&Path]) -> Server {
    let port = free_port();
    let mut command = Command::new(env!("CARGO_BIN_EXE_wiferry"));
    command.args([
        "--no-browser",
        "--host-ip",
        "127.0.0.1",
        "--port",
        &port.to_string(),
        "--name",
        "Integration Host",
    ]);
    for source in sources {
        command.arg("--file").arg(source);
    }
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut management_url = String::new();
    let mut guest_url = String::new();
    for _ in 0..8 {
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        let line = line.trim_end();
        if let Some(value) = line.strip_prefix("Wiferry Rust management:") {
            management_url = value.trim().to_string();
        }
        if let Some(value) = line.strip_prefix("Guest URL:") {
            guest_url = value.trim().to_string();
        }
        if !management_url.is_empty() && !guest_url.is_empty() {
            break;
        }
    }
    assert!(!management_url.is_empty());
    assert!(!guest_url.is_empty());
    let (admin_base, admin_token) = management_url.split_once('#').unwrap();
    let guest_token = guest_url
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap()
        .to_string();
    Server {
        child,
        _stdout: stdout,
        admin_base: admin_base.to_string(),
        admin_token: admin_token.to_string(),
        guest_base: format!("http://127.0.0.1:{port}/api/session/{guest_token}"),
        guest_token,
    }
}

fn admin_api(server: &Server, path: &str) -> String {
    format!("{}api/admin/{path}", server.admin_base)
}

fn state(server: &Server) -> Value {
    let body = client()
        .get(&server.guest_base)
        .send()
        .unwrap()
        .error_for_status()
        .unwrap()
        .text()
        .unwrap();
    serde_json::from_str(&body).unwrap()
}

fn file_id(manifest: &Value, name: &str) -> String {
    manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["name"] == name)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn rejects_an_advertised_address_not_owned_by_the_host() {
    let output = Command::new(env!("CARGO_BIN_EXE_wiferry"))
        .args([
            "--no-browser",
            "--host-ip",
            "203.0.113.9",
            "--port",
            &free_port().to_string(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("must be assigned to this computer"));
}

#[test]
fn rejects_zero_as_an_advertised_port() {
    let output = Command::new(env!("CARGO_BIN_EXE_wiferry"))
        .args(["--no-browser", "--port", "0"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("between 1 and 65535"));
}

#[test]
fn rejects_loopback_as_an_explicit_tailscale_transport() {
    let output = Command::new(env!("CARGO_BIN_EXE_wiferry"))
        .args([
            "--no-browser",
            "--transport",
            "tailscale",
            "--host-ip",
            "127.0.0.1",
            "--port",
            &free_port().to_string(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot use Tailscale transport"));
}

#[test]
fn bundled_host_serves_full_range_head_and_empty_downloads() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("hello 世界.txt");
    let empty = temp.path().join("empty file.bin");
    let content = b"Wiferry Rust integration test.\n".repeat(8192);
    std::fs::write(&source, &content).unwrap();
    std::fs::write(&empty, []).unwrap();
    let server = spawn_server(&[&source, &empty]);
    let manifest = state(&server);
    assert_eq!(manifest["deviceName"], "Integration Host");
    assert_eq!(manifest["features"]["rustCore"], true);
    assert_eq!(manifest["transport"], "lan");
    assert!(manifest.get("hostCandidates").is_none());

    let admin_manifest_body = client()
        .get(admin_api(&server, "state"))
        .header("X-Wiferry-Admin", &server.admin_token)
        .send()
        .unwrap()
        .error_for_status()
        .unwrap()
        .text()
        .unwrap();
    let admin_manifest: Value = serde_json::from_str(&admin_manifest_body).unwrap();
    assert_eq!(admin_manifest["transport"], "lan");
    assert_eq!(admin_manifest["hostIp"], "127.0.0.1");
    assert_eq!(admin_manifest["hostCandidates"][0]["address"], "127.0.0.1");
    assert_eq!(admin_manifest["hostCandidates"][0]["kind"], "lan");
    assert_eq!(admin_manifest["hostCandidates"][0]["label"], "Loopback");

    let source_url = format!(
        "{}/files/{}",
        server.guest_base,
        file_id(&manifest, "hello 世界.txt")
    );
    let full = client()
        .get(&source_url)
        .send()
        .unwrap()
        .error_for_status()
        .unwrap()
        .bytes()
        .unwrap();
    assert_eq!(full.as_ref(), content);
    let range = client()
        .get(&source_url)
        .header(RANGE, "bytes=7-1023")
        .send()
        .unwrap()
        .error_for_status()
        .unwrap();
    assert_eq!(range.status(), 206);
    assert_eq!(
        range.headers()["content-range"],
        format!("bytes 7-1023/{}", content.len())
    );
    assert_eq!(range.bytes().unwrap().as_ref(), &content[7..=1023]);
    let head = client().head(&source_url).send().unwrap();
    assert_eq!(head.status(), 200);
    assert_eq!(head.headers()["content-length"], content.len().to_string());
    assert!(head.bytes().unwrap().is_empty());

    let empty_url = format!(
        "{}/files/{}",
        server.guest_base,
        file_id(&manifest, "empty file.bin")
    );
    let empty_response = client().get(&empty_url).send().unwrap();
    assert_eq!(empty_response.status(), 200);
    assert_eq!(empty_response.headers()["content-length"], "0");
    assert!(empty_response.bytes().unwrap().is_empty());
    let empty_head = client().head(&empty_url).send().unwrap();
    assert_eq!(empty_head.status(), 200);
    assert_eq!(empty_head.headers()["content-length"], "0");
    let empty_range = client()
        .get(&empty_url)
        .header(RANGE, "bytes=0-")
        .send()
        .unwrap();
    assert_eq!(empty_range.status(), 416);
    assert_eq!(empty_range.headers()["content-range"], "bytes */0");

    std::fs::write(&source, b"source changed after sharing").unwrap();
    assert_eq!(client().get(&source_url).send().unwrap().status(), 409);
}

#[test]
fn management_listener_rejects_rebinding_and_requires_capability() {
    let server = spawn_server(&[]);
    let browser_page = client()
        .get(&server.admin_base)
        .send()
        .unwrap()
        .error_for_status()
        .unwrap()
        .text()
        .unwrap();
    assert!(!browser_page.contains(&server.admin_token));

    let rebound = client()
        .get(&server.admin_base)
        .header(HOST, "attacker.example")
        .send()
        .unwrap();
    assert_eq!(rebound.status(), 403);

    let no_capability = client().get(admin_api(&server, "state")).send().unwrap();
    assert_eq!(no_capability.status(), 403);
    let with_capability = client()
        .get(admin_api(&server, "state"))
        .header("X-Wiferry-Admin", &server.admin_token)
        .send()
        .unwrap();
    assert_eq!(with_capability.status(), 200);

    let bad_origin = client()
        .post(admin_api(&server, "stop"))
        .header("X-Wiferry-Admin", &server.admin_token)
        .header(ORIGIN, "http://attacker.example")
        .send()
        .unwrap();
    assert_eq!(bad_origin.status(), 403);
    let guest_admin_route = client()
        .get(format!(
            "http://127.0.0.1:{}/api/admin/state",
            server
                .guest_base
                .split(':')
                .nth(2)
                .unwrap()
                .split('/')
                .next()
                .unwrap()
        ))
        .send()
        .unwrap();
    assert_eq!(guest_admin_route.status(), 404);
}

#[test]
fn rotation_and_stop_revoke_guest_capabilities() {
    let server = spawn_server(&[]);
    let rotated_body = client()
        .post(admin_api(&server, "rotate"))
        .header("X-Wiferry-Admin", &server.admin_token)
        .header(ORIGIN, server.admin_base.trim_end_matches('/'))
        .send()
        .unwrap()
        .error_for_status()
        .unwrap()
        .text()
        .unwrap();
    let rotated: Value = serde_json::from_str(&rotated_body).unwrap();
    let new_share_url = rotated["shareUrl"].as_str().unwrap();
    let new_token = new_share_url
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap();
    assert_ne!(new_token, server.guest_token);
    assert_eq!(
        client().get(&server.guest_base).send().unwrap().status(),
        404
    );
    let new_base = server.guest_base.replace(&server.guest_token, new_token);
    assert_eq!(client().get(&new_base).send().unwrap().status(), 200);

    client()
        .post(admin_api(&server, "stop"))
        .header("X-Wiferry-Admin", &server.admin_token)
        .header(ORIGIN, server.admin_base.trim_end_matches('/'))
        .send()
        .unwrap()
        .error_for_status()
        .unwrap();
    assert_eq!(client().get(&new_base).send().unwrap().status(), 410);
}

#[test]
fn stop_interrupts_an_active_download() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("large sparse payload.bin");
    const SIZE: u64 = 128 * 1024 * 1024;
    std::fs::File::create(&source)
        .unwrap()
        .set_len(SIZE)
        .unwrap();
    let server = spawn_server(&[&source]);
    let manifest = state(&server);
    let download_url = format!(
        "{}/files/{}",
        server.guest_base,
        file_id(&manifest, "large sparse payload.bin")
    );
    let (started_tx, started_rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        let mut response = streaming_client()
            .get(download_url)
            .send()
            .unwrap()
            .error_for_status()
            .unwrap();
        let mut total = 0_u64;
        let mut first = true;
        let mut interrupted = false;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            match response.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    total += read as u64;
                    if first {
                        first = false;
                        started_tx.send(()).unwrap();
                    }
                    thread::sleep(Duration::from_millis(1));
                }
                Err(_) => {
                    interrupted = true;
                    break;
                }
            }
        }
        (total, interrupted)
    });
    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    client()
        .post(admin_api(&server, "stop"))
        .header("X-Wiferry-Admin", &server.admin_token)
        .send()
        .unwrap()
        .error_for_status()
        .unwrap();
    let (received, interrupted) = reader.join().unwrap();
    assert!(interrupted);
    assert!(received < SIZE);
}
