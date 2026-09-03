use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};
use subtle::ConstantTimeEq;
use tempfile::TempDir;
use thiserror::Error;
use tokio::sync::Semaphore;
use unicode_normalization::UnicodeNormalization;

use crate::network::LocalNetwork;

const TOKEN_BYTES: usize = 24;

#[derive(Debug, Error)]
pub enum AccessError {
    #[error("transfer session not found")]
    NotFound,
    #[error("this transfer session has ended")]
    Ended,
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub mime: String,
    pub owned: bool,
    pub modified_nanos: u128,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicFile {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub size_label: String,
    pub mime: String,
}

impl FileEntry {
    pub fn public(&self) -> PublicFile {
        PublicFile {
            id: self.id.clone(),
            name: self.name.clone(),
            size: self.size,
            size_label: human_size(self.size),
            mime: self.mime.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Features {
    pub receive: bool,
    pub download_all: bool,
    pub path_entry: bool,
    pub rust_core: bool,
    pub lan_guard: bool,
    pub activity: bool,
    pub connected_devices: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicState {
    pub active: bool,
    pub mode: &'static str,
    pub device_name: String,
    pub files: Vec<PublicFile>,
    pub seconds_remaining: Option<u64>,
    pub expiry_minutes: u64,
    pub connected_devices: u64,
    pub activities: Vec<serde_json::Value>,
    pub can_download: bool,
    pub can_upload: bool,
    pub features: Features,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub share_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_candidates: Option<Vec<String>>,
}

#[derive(Debug)]
struct Inner {
    token: String,
    admin_token: String,
    active: bool,
    expires_at: Option<u64>,
    expiry_minutes: u64,
    generation: u64,
    files: Vec<FileEntry>,
    host_ip: Ipv4Addr,
    host_candidates: Vec<Ipv4Addr>,
}

#[derive(Debug)]
pub struct AppState {
    inner: RwLock<Inner>,
    pub port: u16,
    pub admin_port: u16,
    pub device_name: String,
    pub temp: TempDir,
    pub networks: Vec<LocalNetwork>,
    pub upload_slots: Semaphore,
}

#[derive(Debug, Clone)]
pub struct Authorization {
    pub generation: u64,
}

impl AppState {
    pub fn new(
        host_ip: Ipv4Addr,
        host_candidates: Vec<Ipv4Addr>,
        networks: Vec<LocalNetwork>,
        port: u16,
        admin_port: u16,
        device_name: String,
        expiry_minutes: u64,
    ) -> std::io::Result<Self> {
        let temp = tempfile::Builder::new().prefix("wiferry-rust-").tempdir()?;
        Ok(Self {
            inner: RwLock::new(Inner {
                token: random_token(TOKEN_BYTES),
                admin_token: random_token(TOKEN_BYTES),
                active: true,
                expires_at: expiry_timestamp(expiry_minutes),
                expiry_minutes,
                generation: 0,
                files: Vec::new(),
                host_ip,
                host_candidates,
            }),
            port,
            admin_port,
            device_name,
            temp,
            networks,
            upload_slots: Semaphore::new(2),
        })
    }

    pub fn admin_token(&self) -> String {
        self.inner.read().unwrap().admin_token.clone()
    }

    pub fn share_url(&self) -> String {
        let inner = self.inner.read().unwrap();
        format!("http://{}:{}/s/{}/", inner.host_ip, self.port, inner.token)
    }

    pub fn public_state(&self, admin: bool) -> PublicState {
        let inner = self.inner.read().unwrap();
        let active = is_active(&inner);
        PublicState {
            active,
            mode: "send",
            device_name: self.device_name.clone(),
            files: inner.files.iter().map(FileEntry::public).collect(),
            seconds_remaining: inner
                .expires_at
                .map(|expires| expires.saturating_sub(now())),
            expiry_minutes: inner.expiry_minutes,
            connected_devices: 0,
            activities: Vec::new(),
            can_download: active,
            can_upload: false,
            features: Features {
                receive: false,
                download_all: false,
                path_entry: true,
                rust_core: true,
                lan_guard: true,
                activity: false,
                connected_devices: false,
            },
            share_url: admin
                .then(|| format!("http://{}:{}/s/{}/", inner.host_ip, self.port, inner.token)),
            host_ip: admin.then(|| inner.host_ip.to_string()),
            host_candidates: admin.then(|| {
                inner
                    .host_candidates
                    .iter()
                    .map(ToString::to_string)
                    .collect()
            }),
        }
    }

    pub fn authorize(&self, token: &str) -> Result<Authorization, AccessError> {
        let inner = self.inner.read().unwrap();
        if inner.token.as_bytes().ct_eq(token.as_bytes()).unwrap_u8() != 1 {
            return Err(AccessError::NotFound);
        }
        if !is_active(&inner) {
            return Err(AccessError::Ended);
        }
        Ok(Authorization {
            generation: inner.generation,
        })
    }

    pub fn authorization_active(&self, token: &str, generation: u64) -> bool {
        let inner = self.inner.read().unwrap();
        inner.token.as_bytes().ct_eq(token.as_bytes()).unwrap_u8() == 1
            && inner.generation == generation
            && is_active(&inner)
    }

    pub fn add_path(&self, raw: &Path, owned: bool) -> Result<PublicFile, String> {
        let path = raw
            .canonicalize()
            .map_err(|_| format!("File does not exist: {}", raw.display()))?;
        let metadata = path
            .metadata()
            .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
        if !metadata.is_file() {
            return Err(format!("Not a regular file: {}", path.display()));
        }
        let raw_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("shared-file")
            .to_string();
        let mut inner = self.inner.write().unwrap();
        let name = unique_name(
            safe_filename(&raw_name),
            inner.files.iter().map(|file| &file.name),
        );
        let modified_nanos = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let item = FileEntry {
            id: random_token(12),
            name,
            path,
            size: metadata.len(),
            mime: mime_guess::from_path(&raw_name)
                .first_or_octet_stream()
                .to_string(),
            owned,
            modified_nanos,
        };
        let public = item.public();
        inner.files.push(item);
        Ok(public)
    }

    pub fn file(&self, id: &str) -> Option<FileEntry> {
        self.inner
            .read()
            .unwrap()
            .files
            .iter()
            .find(|file| file.id == id)
            .cloned()
    }

    pub fn remove(&self, id: &str) -> bool {
        let mut inner = self.inner.write().unwrap();
        let Some(index) = inner.files.iter().position(|file| file.id == id) else {
            return false;
        };
        let item = inner.files.remove(index);
        inner.generation += 1;
        drop(inner);
        if item.owned {
            let _ = fs::remove_file(item.path);
        }
        true
    }

    pub fn clear(&self) {
        let ids: Vec<String> = self
            .inner
            .read()
            .unwrap()
            .files
            .iter()
            .map(|file| file.id.clone())
            .collect();
        for id in ids {
            self.remove(&id);
        }
    }

    pub fn stop(&self) {
        let mut inner = self.inner.write().unwrap();
        inner.active = false;
        inner.generation += 1;
    }

    pub fn start_or_rotate(&self) {
        let mut inner = self.inner.write().unwrap();
        inner.token = random_token(TOKEN_BYTES);
        inner.active = true;
        inner.generation += 1;
        inner.expires_at = expiry_timestamp(inner.expiry_minutes);
    }

    pub fn set_expiry(&self, minutes: u64) -> Result<(), String> {
        if !matches!(minutes, 0 | 15 | 30 | 60 | 120) {
            return Err("Unsupported expiry".into());
        }
        let mut inner = self.inner.write().unwrap();
        let session_was_active = is_active(&inner);
        inner.expiry_minutes = minutes;
        if session_was_active {
            inner.expires_at = expiry_timestamp(minutes);
        }
        inner.generation += 1;
        Ok(())
    }

    pub fn set_host_ip(&self, address: Ipv4Addr) -> Result<(), String> {
        let mut inner = self.inner.write().unwrap();
        if !inner.host_candidates.contains(&address) {
            return Err("Choose an address assigned to this computer".into());
        }
        inner.host_ip = address;
        inner.token = random_token(TOKEN_BYTES);
        inner.generation += 1;
        Ok(())
    }

    pub fn temp_target(&self, name: &str) -> Result<PathBuf, String> {
        let base = safe_filename(name);
        for attempt in 1..10_000 {
            let candidate_name = if attempt == 1 {
                base.clone()
            } else {
                numbered_name(&base, attempt)
            };
            let candidate = self.temp.path().join(candidate_name);
            if !candidate.exists() {
                return Ok(candidate);
            }
        }
        Err("Could not allocate a temporary filename".into())
    }
}

fn is_active(inner: &Inner) -> bool {
    inner.active && inner.expires_at.is_none_or(|expires| expires > now())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn expiry_timestamp(minutes: u64) -> Option<u64> {
    (minutes > 0).then(|| now() + minutes * 60)
}

fn random_token(size: usize) -> String {
    let mut bytes = vec![0_u8; size];
    getrandom::fill(&mut bytes).expect("operating-system random generator unavailable");
    URL_SAFE_NO_PAD.encode(bytes)
}

fn human_size(size: u64) -> String {
    let mut value = size as f64;
    for unit in ["B", "KB", "MB", "GB", "TB"] {
        if value < 1024.0 || unit == "TB" {
            return if matches!(unit, "B" | "KB") {
                format!("{value:.0} {unit}")
            } else {
                format!("{value:.1} {unit}")
            };
        }
        value /= 1024.0;
    }
    format!("{size} B")
}

fn safe_filename(input: &str) -> String {
    let normalized_path = input.replace('\\', "/");
    let leaf = normalized_path.rsplit('/').next().unwrap_or("shared-file");
    let mut value: String = leaf
        .nfc()
        .map(|character| {
            if character.is_control() || r#"<>:"/\|?*"#.contains(character) {
                '_'
            } else {
                character
            }
        })
        .collect();
    value = value.trim_matches([' ', '.']).to_string();
    if value.is_empty() || matches!(value.as_str(), "." | "..") {
        value = "shared-file".into();
    }
    let stem_upper = value
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = matches!(stem_upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem_upper.len() == 4
            && (stem_upper.starts_with("COM") || stem_upper.starts_with("LPT"))
            && stem_upper[3..]
                .parse::<u8>()
                .is_ok_and(|number| (1..=9).contains(&number)));
    if reserved {
        value.insert(0, '_');
    }
    truncate_utf8(&value, 240)
}

fn truncate_utf8(value: &str, budget: usize) -> String {
    if value.len() <= budget {
        return value.to_string();
    }
    let mut end = budget;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

fn unique_name<'a>(base: String, existing: impl Iterator<Item = &'a String>) -> String {
    let names: HashSet<&str> = existing.map(String::as_str).collect();
    if !names.contains(base.as_str()) {
        return base;
    }
    for number in 2..10_000 {
        let candidate = numbered_name(&base, number);
        if !names.contains(candidate.as_str()) {
            return candidate;
        }
    }
    format!("{}-{}", base, random_token(4))
}

fn numbered_name(base: &str, number: usize) -> String {
    let path = Path::new(base);
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(base);
    if extension.is_empty() {
        format!("{stem} ({number})")
    } else {
        format!("{stem} ({number}).{extension}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_portable_names() {
        assert_eq!(safe_filename("../../CON.txt"), "_CON.txt");
        assert_eq!(safe_filename("bad<name>?.txt"), "bad_name__.txt");
        assert!(safe_filename(&("😀".repeat(100) + ".txt")).len() <= 240);
    }

    #[test]
    fn expiry_and_rotation_invalidate_authorization() {
        let state = AppState::new(
            Ipv4Addr::LOCALHOST,
            vec![Ipv4Addr::LOCALHOST],
            Vec::new(),
            8765,
            8766,
            "Test host".into(),
            0,
        )
        .unwrap();
        let token = state.inner.read().unwrap().token.clone();
        let authorization = state.authorize(&token).unwrap();
        state.start_or_rotate();
        assert!(!state.authorization_active(&token, authorization.generation));
        assert!(matches!(
            state.authorize(&token),
            Err(AccessError::NotFound)
        ));

        let current = state.inner.read().unwrap().token.clone();
        state.inner.write().unwrap().expires_at = Some(now().saturating_sub(1));
        assert!(matches!(state.authorize(&current), Err(AccessError::Ended)));
        state.set_expiry(15).unwrap();
        assert!(matches!(state.authorize(&current), Err(AccessError::Ended)));
        state.start_or_rotate();
        assert!(matches!(
            state.authorize(&current),
            Err(AccessError::NotFound)
        ));
    }
}
