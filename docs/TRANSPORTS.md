# Wiferry transport and mobile model

Wiferry follows one product rule:

> Wiferry works without an app; the app makes frequent transfers better.

The receiver remains browser-first. An optional native mobile companion may add
system integration later, but it is not a prerequisite for opening a shared file.

## Transport layers

| Mode | Intended reach | Required on receiver | File path |
|---|---|---|---|
| Nearby | One selected LAN or private VPN subnet | Browser | Direct HTTP |
| Tailnet | An existing Tailscale tailnet | Tailscale plus browser | HTTP inside Tailscale WireGuard |
| Anywhere, planned experiment (not implemented) | Temporary cross-network session | Undecided | Future Tailcat/WebRTC adapter |

### Nearby

Nearby is the default. Wiferry advertises one local address and accepts a guest
only when its transport peer belongs to that selected interface subnet. Choosing
one interface does not implicitly authorize peers from every other Wi-Fi, VPN,
or virtual adapter on the host.

The browser-facing URL is HTTP. Use this mode only on a trusted LAN or personal
hotspot. The random Wiferry URL is authorization, not encryption.

### Tailnet

Tailnet mode is for people who already use Tailscale. Wiferry confirms a local
Tailscale IPv4 address, advertises that address in the QR, and admits only
loopback or source addresses in Tailscale's `100.64.0.0/10` device range. This
is not identity authentication; Tailscale ACLs or grants provide that layer. The receiving
phone or computer needs access to the same tailnet but does not need Wiferry.

In `auto` and `tailscale` modes, Wiferry probes `tailscale ip -4` from `PATH`
with a 750 ms deadline and intersects the result with assigned interfaces. An
explicitly named Tailscale adapter is the cross-platform fallback. Explicit
`lan` startup skips the CLI probe, so a stalled Tailscale command cannot block
the LAN workflow. Address discovery is a startup snapshot in this alpha.

Tailscale ACLs or grants and the Wiferry capability are separate layers. The
page still has an `http://100.x` URL, while packets travel inside Tailscale's
encrypted WireGuard tunnel. Tailscale can use a direct path, or DERP may relay
traffic that remains WireGuard-encrypted. Wiferry operates neither a cloud store
nor that relay.

MagicDNS and [Tailscale Serve](https://tailscale.com/docs/features/tailscale-serve)
could add a trusted `.ts.net` HTTPS endpoint later. Identity headers must never
be trusted by a backend that is also directly reachable outside the Serve proxy.

### Tailcat, planned experiment (not implemented)

[Tailcat](https://tailscale.com/blog/tailcat) is a separate, recently opened
Tailscale project. It uses a `tc…` bearer address, userspace WireGuard, NAT
traversal, and DERP fallback without a Tailscale account or control plane. It is
not the same thing as using an existing `100.x` tailnet address.

Wiferry does not embed Tailcat in the default Rust host because:

- its Go API, CLI, and wire format currently have no stability promise;
- its browser WebAssembly path is experimental and currently DERP-only;
- the hosted Tailcat DERP service is rate-limited and has no uptime or throughput SLA;
- its security guidance says mutually untrusted peers are not yet the fully
  hardened historical threat model;
- the browser payload would materially outweigh Wiferry's current host binary.

A future adapter should be optional, pin an exact reviewed Tailcat tag or commit,
use ephemeral keys, expose only Wiferry file streams, and never expose Tailcat SSH, exit-node,
or arbitrary-port capabilities. A zero-install browser adapter must put the
`tc…` secret in a URL fragment, remove it immediately, disclose DERP metadata,
and offer self-hosted DERP.

## Mobile flows

### Computer to phone

This is the current workflow: run Wiferry on the computer and scan from Safari
or Chrome. No Wiferry mobile app is required.

### Phone to computer

The planned Desktop Inbox reverses the page permissions. The computer displays
an upload-only QR; the phone selects Files or Photos in its browser; Wiferry
streams into a host-selected directory with a quota, temporary filename, atomic
completion, expiry, and immediate revocation. No phone app is required.

### iPhone to Android through a computer

The planned Bridge Room uses two capabilities:

1. iPhone scans an upload-only QR and sends to the computer Inbox.
2. The computer owner explicitly approves the received file for forwarding.
3. Android scans a separate download-only QR.

Neither phone installs Wiferry. Host approval prevents an untrusted uploader
from making a malicious file appear to be a file selected by the computer owner.

### Phone to phone without a computer

A foreground HTTPS WebRTC page can transfer chunks between browsers without an
installed app, but it needs signaling and usually STUN/TURN. Mobile browsers may
suspend a tab during screen lock or app switching, so this should remain a
best-effort path until real iOS and Android interruption tests pass.

An optional sender-first native companion can later add iOS Share Extension,
Android share intents, background jobs, notifications, and stronger resume. The
other device should still be able to receive in its browser.

## Installation policy

- A one-time receiver is never forced to install Wiferry.
- A phone using Tailnet already needs Tailscale, but not a Wiferry client.
- Frequent senders may optionally install a Wiferry companion for background
  and operating-system integration.
- The transfer page contains no ads, analytics, or third-party tracking SDKs.
