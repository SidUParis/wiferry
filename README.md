# Wiferry

**One host binary. Any nearby browser. No cloud relay.**

Wiferry is an MIT-licensed local-network file delivery tool. Run it on a Linux,
macOS, or Windows computer, add files by path or drag-and-drop, then let any
nearby phone, tablet, TV, or computer scan an expiring QR capability link and
download through its normal browser.

> Status: `0.2.0-alpha.1`. The Rust host is implemented and verified on Linux
> x86-64. Windows and macOS source/build jobs are included but must pass their
> first public GitHub Actions run before their binaries are called verified.

## Why another LAN file tool?

The category is established: LocalSend, qrcp, croc, PairDrop, QRDrop, `drop`,
and Su-Share all solve overlapping parts. Rust, QR codes, browser receivers,
and HTTP Range are not individually novel.

Wiferry focuses on a measurable combination:

- **One installed side:** the host is a small binary; every guest is browser-only.
- **Explicit no-copy path:** CLI and local path entry share the original file in
  place. Browser drag-and-drop remains a bounded local copy because browsers do
  not reveal native paths.
- **Application-layer LAN guard:** guest requests must originate from an actual
  local interface subnet; forwarded headers are ignored.
- **Revocable capability:** 192-bit random URL, expiry, rotation, loopback-only
  management, and generation checks that terminate an active response.
- **Published benchmark:** binary size, cold start, process-tree RSS, throughput,
  and checksum correctness are measured by the reproducible comparison runner.
  Active-stream cancellation is outside that runner and is not a benchmark result.

## Measured Linux result

On one Ubuntu x86-64 host, using a 256 MiB file and five loopback runs:

| Metric | Legacy Python bundle | Rust core | Result |
|---|---:|---:|---:|
| Binary | 22,804,552 B | 2,585,416 B | Rust 88.7% smaller |
| Ready to HTTP | 783.5 ms | 2.7 ms | Rust about 287x faster |
| Idle process-tree RSS | 61,964,288 B | 4,870,144 B | Rust 92.1% lower |
| Loopback throughput | 913.3 MiB/s | 988.1 MiB/s | Rust 8.2% higher in this run |

Every download matched SHA-256. This is a framework-ceiling test, not a Wi-Fi
speed claim. See [`benchmarks/RESULTS-linux-x86_64.md`](benchmarks/RESULTS-linux-x86_64.md)
and the raw JSON. Wiferry describes Rust as *lighter and faster to become
ready*; the noisy loopback result is not a claim of universally higher network
throughput.

## Use

Share paths in place (recommended for large files):

```bash
wiferry report.pdf demo.mp4
wiferry --file report.pdf --file demo.mp4
```

Or start without paths and use the local management page:

```bash
wiferry
```

The browser interface supports:

- file drag-and-drop or picker, copied locally with a 2 GiB request limit;
- a loopback-only path field for in-place sharing;
- QR generation, address selection for VPN/multi-NIC hosts, expiry, rotation,
  removal, and immediate stop.

Useful options:

```text
--port 8765
--host-ip 192.168.1.50
--name "My laptop"
--expiry 15|30|60|120|0
--no-browser
```

## Build the Rust host

Requirements: Rust 1.98.1 and Node.js 22+.

```bash
npm --prefix frontend ci
npm --prefix frontend run build
cargo build --release --locked
./target/release/wiferry --help
```

The Vite output is embedded directly into the Rust executable at compile time;
Node is not required at runtime. `Cargo.lock` is committed, and CI always builds
with `--locked`.

## Security boundary

The QR URL is a capability, not encryption. Anyone who can read it can download
until it expires or is revoked. The management server binds a separate
loopback-only port. Its random admin capability is delivered in a launch URL
fragment, removed from browser history, and required by every management API
call; it is never embedded in an HTTP response. Management requests also enforce
an allowlisted loopback `Host` authority, and mutations reject foreign browser origins. Guest
peers are checked against locally enumerated subnets instead of trusting
`X-Forwarded-For`.

Wiferry 0.2 still uses HTTP because arbitrary phone browsers will not trust a
host-generated certificate. Use a trusted home, office, classroom, or personal
hotspot network. Do not use this alpha for sensitive files on hostile public
Wi-Fi. A PIN/approval mode and an encrypted protocol are roadmap items.

## Repository layout

- `src/`: Rust/Axum/Tokio host, LAN guard, capabilities, and bounded streaming.
- `frontend/`: React/Vite management and guest interface.
- `benchmarks/`: reproducible legacy-vs-Rust measurements and raw results.
- `tests/` and `wiferry/`: the Python reference engine and black-box behavior tests.
- `docs/PRODUCT_SCOPE.md`: scope and acceptance boundary.
- `.github/workflows/`: three-OS CI and native prerelease packaging.

The Python implementation is retained temporarily as a differential reference.
It will move under `legacy-python/` after Rust contract parity is complete.

## Current compatibility boundary

- Linux x86-64 Rust binary: built and locally verified.
- Windows x86-64: source and native CI job prepared; public runner verification pending.
- macOS Apple Silicon and Intel: native CI jobs are the release target; public
  runner verification, signing, and notarization are pending.
- Receiver browsers: the current iPhone/Safari LAN path and desktop browser path
  have been exercised; broader browser/device results will be published as a matrix.

## Community

See [`CONTRIBUTING.md`](CONTRIBUTING.md), [`ROADMAP.md`](ROADMAP.md), and
[`SECURITY.md`](SECURITY.md). Performance claims require a committed benchmark
result and method. Please do not open public issues for undisclosed security
reports; use GitHub private vulnerability reporting.

## License

MIT. See [`LICENSE`](LICENSE).
