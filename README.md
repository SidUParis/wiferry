# Wiferry

**One host binary. Any authorized browser. No Wiferry cloud storage.**

Wiferry is an MIT-licensed direct file delivery tool. Run it on a Linux, macOS,
or Windows computer, add files by path or drag-and-drop, then let a phone,
tablet, TV, or computer scan an expiring QR capability link and download through
its normal browser. Use Nearby mode on one LAN or Tailnet mode between devices
already connected through Tailscale.

> Source status: `0.2.0-alpha.2` candidate. The released alpha.1 passed
> [native four-platform CI](https://github.com/SidUParis/wiferry/actions/runs/33819199921).
> The alpha.2 Tailnet change passed a
> [real full-file, Range, and transport-scope test](docs/TAILNET_VALIDATION.md)
> from a second Linux device; its new public matrix is still a release gate.

## Why another LAN file tool?

The category is established: LocalSend, qrcp, croc, PairDrop, QRDrop, `drop`,
and Su-Share all solve overlapping parts. Rust, QR codes, browser receivers,
and HTTP Range are not individually novel.

Wiferry focuses on a measurable combination:

- **One installed side:** the host is a small binary; every guest is browser-only
  with respect to Wiferry. Tailnet guests separately need Tailscale access.
- **Explicit no-copy path:** CLI and local path entry share the original file in
  place. Browser drag-and-drop remains a bounded local copy because browsers do
  not reveal native paths.
- **Transport-scoped guard:** Nearby accepts only the selected interface subnet.
  After confirming the host's Tailscale address, Tailnet admits only loopback or
  source addresses in Tailscale's `100.64.0.0/10` device range. Tailscale policy
  remains responsible for peer identity. Forwarded headers are ignored.
- **Revocable capability:** 192-bit random URL, expiry, rotation, loopback-only
  management, and generation checks that terminate an active response.
- **Published benchmark:** binary size, cold start, process-tree RSS, throughput,
  and checksum correctness are measured by the reproducible comparison runner.
  Active-stream cancellation is outside that runner and is not a benchmark result.

## Measured Linux result

For the `0.2.0-alpha.1` binary on one Ubuntu x86-64 host, using a 256 MiB file
and five loopback runs:

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

Share through an existing Tailscale network:

```bash
wiferry --transport tailscale report.pdf
```

The receiving phone or computer needs Tailscale access to the same tailnet, but
does not need Wiferry; it still scans the QR and uses its normal browser.

Or start without paths and use the local management page:

```bash
wiferry
```

The browser interface supports:

- file drag-and-drop or picker, copied locally with a 2 GiB request limit;
- a loopback-only path field for in-place sharing;
- QR generation, labeled LAN/VPN/Tailscale address selection, expiry, rotation,
  removal, and immediate stop.

Useful options:

```text
--port 8765
--host-ip 192.168.1.50
--transport auto|lan|tailscale
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
peers are checked against the currently selected LAN or Tailnet policy instead
of trusting `X-Forwarded-For`.

Nearby mode still uses HTTP because arbitrary phone browsers will not trust a
host-generated certificate. Use a trusted home, office, classroom, or personal
hotspot network. Tailnet mode also displays an HTTP URL, but its packets travel
inside Tailscale's encrypted WireGuard tunnel. DERP may relay packets that remain
WireGuard-encrypted.
Wiferry does not provide, operate, or claim that relay. A PIN/approval mode and
an independent encrypted transport are roadmap items.

## Repository layout

- `src/`: Rust/Axum/Tokio host, LAN guard, capabilities, and bounded streaming.
- `frontend/`: React/Vite management and guest interface.
- `benchmarks/`: reproducible legacy-vs-Rust measurements and raw results.
- `tests/` and `wiferry/`: the Python reference engine and black-box behavior tests.
- `docs/PRODUCT_SCOPE.md`: scope and acceptance boundary.
- `docs/TRANSPORTS.md`: Nearby, Tailnet, experimental Tailcat, and mobile model.
- `.github/workflows/`: three-OS CI and native prerelease packaging.

The Python implementation is retained temporarily as a differential reference.
It will move under `legacy-python/` after Rust contract parity is complete.

## Current compatibility boundary

- Linux x86-64: local smoke tests and the native public runner pass. The alpha
  artifact uses GNU/glibc; a broadly static Linux bundle remains future work.
- Windows x86-64: native public build, Clippy, and black-box tests pass.
- macOS Apple Silicon and Intel: both native public build/test jobs pass;
  signing and notarization remain pending.
- Receiver browsers: the current iPhone/Safari LAN path and desktop browser path
  have been exercised; broader browser/device results will be published as a matrix.

## Community

See [`CONTRIBUTING.md`](CONTRIBUTING.md), [`ROADMAP.md`](ROADMAP.md), and
[`SECURITY.md`](SECURITY.md). Performance claims require a committed benchmark
result and method. Please do not open public issues for undisclosed security
reports; use GitHub private vulnerability reporting.

## License

MIT. See [`LICENSE`](LICENSE).
