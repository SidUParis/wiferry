# Wiferry product scope

## Product sentence

Wiferry lets one Linux, macOS, or Windows computer share local files with any
nearby device: choose or drag files, scan a QR code, and download directly over
the same LAN without a cloud relay or receiver installation.

## Core workflow

1. Start Wiferry on a Linux, macOS, or Windows host.
2. Add one or more files by drag-and-drop, file picker, or command-line path.
3. Wiferry creates a random, expiring local URL and renders it as a QR code.
4. A nearby phone, tablet, TV, or computer scans the code in its normal browser.
5. The browser downloads individual files with HTTP range support.
6. The host stops sharing or rotates the code, immediately revoking the session.

## Required properties

- **Cross-platform host:** the same source builds on Linux, macOS, and Windows.
- **Zero-install receiver:** only the host runs Wiferry; guests need a browser.
- **Local-first transfer:** file bytes remain on the LAN and never use Wiferry
  cloud infrastructure.
- **Fast path:** individual files stream from disk with byte-range support and
  are not uploaded to an intermediary service.
- **Simple input:** drag-and-drop, multi-file picker, positional paths, and
  repeatable `--file` arguments.
- **Capability security:** a high-entropy QR URL, explicit expiry, rotation, and
  a management interface that is unavailable from the LAN.
- **Open source:** MIT-licensed source, reproducible frontend build, tests,
  checksums, and public CI/release definitions.

## Optional, secondary capability

Receive mode allows a guest to upload to a host-selected directory. It remains
off during the default Send workflow and is not required for the core product.

## Planned capabilities

- Downloading all shared files as one bounded-streaming ZIP. The current Rust
  host advertises `features.downloadAll: false`, so guests download files
  individually until this is implemented and verified.

## Explicit non-goals for 0.2 alpha

- Implementing Apple's proprietary AirDrop/AWDL protocol.
- Relaying files over the public internet.
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
