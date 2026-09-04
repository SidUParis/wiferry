# Roadmap

## 0.2 alpha: Rust host parity

- [x] Embedded browser UI and single Rust executable
- [x] In-place CLI/path sharing and bounded browser-copy upload
- [x] Full and single-Range downloads with checksum verification
- [x] Expiring 192-bit capability and active-stream revocation
- [x] Application-layer local-subnet guard
- [x] Public benchmark runner and first Linux result
- [ ] Black-box API contract suite shared by the Python and Rust engines
- [x] First native CI pass on Linux, Windows, macOS arm64, and macOS x86-64
- [ ] Device matrix: iOS Safari, Android Chrome, desktop browsers

## 0.2 alpha.2: Tailnet transport

- [x] Detect a verified local Tailscale IPv4 address
- [x] Scope guest authorization to the selected LAN or Tailnet transport
- [x] Label LAN/VPN/Tailscale choices and rotate the capability on changes
- [x] Real second-device Tailnet full-file and Range checksum test
- [ ] Linux, Windows, and both macOS CI targets for alpha.2
- [ ] Optional MagicDNS and Tailscale Serve HTTPS exploration

## 0.3: Mobile-first Inbox and Bridge

- Rust Receive/Inbox mode for phone-to-computer browser uploads
- Separate upload and download capabilities with host approval
- Desktop Bridge Room for iPhone-to-Android transfers without either phone
  installing Wiferry
- Foreground browser-to-browser WebRTC experiment for transfers without a computer
- Optional sender-first iOS/Android companion for Share Sheet and background jobs
- Experimental Tailcat adapter kept outside the default Rust binary

## 0.4: Reliable resume protocol

- Strong content ETag and `If-Range`
- Stable resumable session manifest and file-change rejection
- Interrupted Wi-Fi, sleep/resume, and browser-refresh test vectors
- Bounded streaming ZIP or a documented per-file-only large-transfer path
- Optional maximum completed downloads and one-time capability mode

## 0.5: Trust on less-trusted networks

- Explicit receiver approval or short PIN
- Authenticated encrypted session design with an external review
- Rate limits and per-peer quotas
- Signed SBOM and build provenance

## Stable release gates

- Signed Windows executable and notarized macOS arm64/x86-64 artifacts
- Reproducible release documentation and post-publish artifact verification
- Independent security review
- No performance claim without current public data
