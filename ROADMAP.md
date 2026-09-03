# Roadmap

## 0.2 alpha: Rust host parity

- [x] Embedded browser UI and single Rust executable
- [x] In-place CLI/path sharing and bounded browser-copy upload
- [x] Full and single-Range downloads with checksum verification
- [x] Expiring 192-bit capability and active-stream revocation
- [x] Application-layer local-subnet guard
- [x] Public benchmark runner and first Linux result
- [ ] Black-box API contract suite shared by the Python and Rust engines
- [ ] First native CI pass on Linux, Windows, macOS arm64, and macOS x86-64
- [ ] Device matrix: iOS Safari, Android Chrome, desktop browsers

## 0.3: Reliable resume protocol

- Strong content ETag and `If-Range`
- Stable resumable session manifest and file-change rejection
- Interrupted Wi-Fi, sleep/resume, and browser-refresh test vectors
- Bounded streaming ZIP or a documented per-file-only large-transfer path
- Optional maximum completed downloads and one-time capability mode

## 0.4: Trust on less-trusted LANs

- Explicit receiver approval or short PIN
- Authenticated encrypted session design with an external review
- Rate limits and per-peer quotas
- Signed SBOM and build provenance

## Stable release gates

- Signed Windows executable and notarized macOS arm64/x86-64 artifacts
- Reproducible release documentation and post-publish artifact verification
- Independent security review
- No performance claim without current public data
