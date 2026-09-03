# Competitive landscape and positioning

Wiferry does not claim that Rust, QR codes, browser receivers, LAN transfer, or
HTTP Range are individually new. The project competes on a measured combination
of distribution size, one-sided installation, explicit capability security,
application-layer LAN restriction, active revocation, and an auditable protocol.

Snapshot date: 2026-09-04.

| Project | Installed sides | Relevant overlap | Wiferry focus |
|---|---|---|---|
| [LocalSend 1.18.2](https://github.com/localsend/localsend/tree/v1.18.2) | Usually both; browser link is optional | Mature cross-platform UI, Rust network core, QR/PIN, TLS for app peers | Browser-only guest as the primary workflow; smaller purpose-built host; explicit LAN/capability benchmark |
| [qrcp 0.11.6](https://github.com/claudiodangelis/qrcp/tree/v0.11.6) | Host CLI only | Small Go binary, QR, browser, Range | Management UI, 192-bit capability, expiry/rotation, LAN guard, active revocation |
| [croc 11.3.6](https://github.com/schollz/croc/tree/v11.3.6) | Usually both; web mode exists | PAKE, encryption, resume, relay fallback | Deliberately local-only and lower-complexity browser delivery; no relay or account |
| [PairDrop](https://github.com/schlagmichdoch/PairDrop) | Browser on both | WebRTC transfer, QR pairing/rooms, broad reach | No signaling service; host owns the bytes and capability lifecycle |
| [QRDrop 0.3.4](https://github.com/behnamazimi/qrdrop/tree/v0.3.4) | Host binary only | QR, browser guest, Range, upload/download | Constant-memory Rust data plane, much smaller measured host, subnet gate and stronger capability defaults |
| [drop 1.1.2](https://github.com/xxeisenberg/drop) | Rust CLI host | Small Rust binary, QR, browser, mDNS | Polished local management UI, explicit security model and benchmark/release gates |
| [Su-Share 1.2.1](https://github.com/Hunter-Lies/Su-Share/tree/v1.2.1) | Tauri host | Rust desktop drag/drop, QR, Range | Cross-platform completion, no WebView requirement, public protocol and measured resource budgets |

## Defensible differentiators

1. A receiver never installs Wiferry; the QR capability URL is the full entry point.
2. A path-based share reads the original file and keeps buffering bounded at
   128 KiB; browser drop is explicitly labeled as a local-copy path.
3. The server rejects guest peers outside locally enumerated interface subnets
   instead of relying only on a random URL or firewall.
4. Stop/rotation/expiry are checked during an active response, not only when a
   request begins.
5. Performance language is gated by committed raw results. The first result
   proves smaller startup/RSS/binary size while also reporting lower Rust
   loopback bulk throughput instead of hiding it.
6. The guest HTTP contract is short enough for third-party clients and test
   vectors, rather than being inseparable from the UI.

## Claims Wiferry intentionally avoids

- “first QR file-transfer tool”
- “first Rust implementation”
- “fastest LAN transfer”
- “encrypted” while the browser flow is HTTP
- “resumable protocol” until strong `ETag`/`If-Range` and interruption tests land
