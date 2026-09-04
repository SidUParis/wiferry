# Changelog

## 0.2.0-alpha.2 - 2026-09-04

- Added explicit `auto`, `lan`, and `tailscale` transports.
- Added verified Tailscale IPv4 discovery and labeled Tailnet QR addresses.
- Scoped LAN access to the selected interface instead of the union of every
  local Wi-Fi, VPN, and virtual adapter.
- Scoped Tailnet access to loopback and Tailscale's `100.64.0.0/10` peer range.
- Made address changes rotate the capability and active-stream generation with
  the selected transport policy.
- Added Tailnet-aware browser copy and a documented mobile Inbox/Bridge model.

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
- Verified native Linux, Windows, macOS arm64, and macOS x86-64 CI builds.

## 0.1.0 - Prototype

- Added the FastAPI/Python reference host and React/Vite interface.
- Added QR capability links, file upload/download, Range, expiry, and rotation.
