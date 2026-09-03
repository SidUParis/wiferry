# Changelog

## 0.2.0-alpha.1 - 2026-09-04

- Added the Rust/Axum/Tokio host and embedded web interface.
- Added application-layer LAN subnet enforcement.
- Split management onto a loopback-only listener with fragment-delivered admin
  capability, allowlisted Host validation, and browser Origin checks.
- Added bounded 128 KiB download streaming and generation-based revocation.
- Added zero-copy path sharing from CLI and loopback management UI.
- Added multi-interface QR address selection.
- Added reproducible Python-versus-Rust size, startup, RSS, and throughput benchmark.
- Made embedded frontend timestamps deterministic for repeatable same-environment builds.
- Prepared native Linux, Windows, macOS arm64, and macOS x86-64 prerelease builds.

## 0.1.0 - Prototype

- Added the FastAPI/Python reference host and React/Vite interface.
- Added QR capability links, file upload/download, Range, expiry, and rotation.
