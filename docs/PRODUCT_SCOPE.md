# Wiferry product scope

## Product sentence

Wiferry lets one Linux, macOS, or Windows computer share files with a browser:
choose or drag files, scan a QR code, and download directly over a selected LAN
or an existing Tailscale tailnet without Wiferry cloud storage or installing
Wiferry on the receiver. Tailnet receivers separately need Tailscale access.

## Core workflow

1. Start Wiferry on a Linux, macOS, or Windows host.
2. Add one or more files by drag-and-drop, file picker, or command-line path.
3. Wiferry creates a random, expiring guest URL and renders it as a QR code.
4. A phone, tablet, TV, or computer on the authorized LAN or tailnet scans the
   code in its normal browser.
5. The browser downloads individual files with HTTP range support.
6. The host stops sharing or rotates the code, immediately revoking the session.

## Required properties

- **Cross-platform host:** the same source builds on Linux, macOS, and Windows.
- **No Wiferry receiver install:** only the host runs Wiferry; Nearby guests need
  a browser, while Tailnet guests need both a browser and existing Tailscale access.
- **Local-first transfer:** Nearby bytes remain on the LAN; Tailnet bytes use
  Tailscale's encrypted path and may use its DERP fallback. Neither mode uses
  Wiferry cloud storage or a Wiferry file relay.
- **Fast path:** individual files stream from disk with byte-range support and
  are not uploaded to an intermediary service.
- **Simple input:** drag-and-drop, multi-file picker, positional paths, and
  repeatable `--file` arguments.
- **Capability security:** a high-entropy QR URL, explicit expiry, rotation, and
  a management interface that is unavailable from the LAN.
- **Scoped transport:** choosing a LAN address accepts only that interface
  subnet; after confirming the host's Tailscale address, Tailnet accepts only
  loopback and source IPv4 addresses in `100.64.0.0/10`. Tailscale policy, not
  this range check, authenticates the peer identity.
- **Open source:** MIT-licensed source, reproducible frontend build, tests,
  checksums, and public CI/release definitions.

## Optional, secondary capability

Receive mode allows a guest to upload to a host-selected directory. It remains
off during the default Send workflow and is not required for the core product.

## Planned capabilities

- Downloading all shared files as one bounded-streaming ZIP. The current Rust
  host advertises `features.downloadAll: false`, so guests download files
  individually until this is implemented and verified.
- A desktop Inbox and two-capability Bridge Room for phone-to-computer and
  iPhone-to-Android transfers without requiring Wiferry on either phone.
- An experimental Tailcat/WebRTC path for temporary transfers without an
  existing tailnet. It is not part of the default binary in this alpha.

## Explicit non-goals for 0.2 alpha

- Implementing Apple's proprietary AirDrop/AWDL protocol.
- Operating a Wiferry public file relay.
- Accounts, contact discovery, cloud history, or analytics.
- Claiming encrypted transport on arbitrary LANs; 0.2 alpha is for trusted networks.
- Shipping signed/notarized installers before project signing identities exist.

## Acceptance checklist

- A release tag builds and smoke-tests an executable on all three host OSes.
- Starting with no path opens a working drag-and-drop management screen.
- Starting with file paths exposes those files without copying them first.
- A current iPhone/Android/desktop browser can scan the QR and list files.
- A full download and a range download match the source bytes.
- Invalid, expired, stopped, and rotated links cannot start new transfers.
- No source path, admin token, or unrelated directory listing reaches guests.
