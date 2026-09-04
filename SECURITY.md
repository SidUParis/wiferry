# Security

## Intended deployment

Wiferry Nearby mode is intended for trusted home, office, classroom, or
personal-hotspot networks and does not provide transport encryption. Tailnet
mode is intended for devices already authorized by the same Tailscale network;
its HTTP packets travel inside Tailscale's encrypted tunnel. Do not use Nearby
mode for sensitive files on untrusted public Wi-Fi.

## Current controls

- Every guest session uses a random 192-bit URL capability.
- Guest links expire and can be revoked or rotated immediately.
- The management server is a separate listener bound only to `127.0.0.1`.
- Management requests require a second random capability delivered in a launch
  URL fragment, never in an HTTP request or HTML response.
- Management routes enforce an allowlisted loopback `Host`; browser mutations reject
  foreign `Origin` values.
- Guest requests are restricted to the selected transport: the chosen LAN
  interface subnet or, after confirming the host's Tailscale address, source
  IPv4 addresses in `100.64.0.0/10`. This range check is not peer identity
  authentication; Tailscale ACLs or grants remain responsible for identity.
  Other local interfaces are not implicitly authorized, and forwarded proxy
  headers are ignored.
- The optional path-entry API is available only to the loopback management page
  with its separate admin capability.
- Uploaded filenames are reduced to a safe basename and collision-renamed.
- Downloads support one validated byte range and never expose directory listings.
- Stop, expiry, and rotation invalidate the generation checked between stream chunks.
- Browser responses disable framing, MIME sniffing, referrers, device permissions,
  cross-origin scripts, and third-party network connections.

## Tailscale boundary

- Wiferry does not treat an arbitrary host `100.x` address as Tailscale; the
  advertised host address must be confirmed locally before Tailnet is offered.
- Tailscale ACLs or grants decide which tailnet nodes can reach the host. The
  Wiferry capability is an additional bearer authorization, not a Tailscale identity.
- DERP may relay Tailscale traffic that remains WireGuard-encrypted. Wiferry
  does not operate that relay or store the file.
- Tailcat is not embedded in the default binary. Any future adapter will remain
  experimental until its evolving wire protocol and mutually-untrusted peer
  model have an independent review.

## Before a stable release

The planned stable security boundary includes mutually authenticated pairing or
session encryption, explicit receiver confirmation for untrusted LANs, upload
quotas, rate limiting, and an external security review.

Report vulnerabilities with GitHub private vulnerability reporting:
https://github.com/SidUParis/wiferry/security/advisories/new

Do not include a live capability URL or sensitive filename in a public issue.
