# Security

## Intended deployment

Wiferry 0.2 alpha is intended for trusted home, office, classroom, or personal-hotspot
networks. It does not provide transport encryption yet. Do not use the alpha for
sensitive files on an untrusted public Wi-Fi.

## Current controls

- Every guest session uses a random 192-bit URL capability.
- Guest links expire and can be revoked or rotated immediately.
- The management server is a separate listener bound only to `127.0.0.1`.
- Management requests require a second random capability delivered in a launch
  URL fragment, never in an HTTP request or HTML response.
- Management routes enforce an allowlisted loopback `Host`; browser mutations reject
  foreign `Origin` values.
- Guest requests are restricted to subnets assigned to local interfaces; forwarded
  proxy headers are ignored.
- The optional path-entry API is available only to the loopback management page
  with its separate admin capability.
- Uploaded filenames are reduced to a safe basename and collision-renamed.
- Downloads support one validated byte range and never expose directory listings.
- Stop, expiry, and rotation invalidate the generation checked between stream chunks.
- Browser responses disable framing, MIME sniffing, referrers, device permissions,
  cross-origin scripts, and third-party network connections.

## Before a stable release

The planned stable security boundary includes mutually authenticated pairing or
session encryption, explicit receiver confirmation for untrusted LANs, upload
quotas, rate limiting, and an external security review.

Report vulnerabilities with GitHub private vulnerability reporting:
https://github.com/SidUParis/wiferry/security/advisories/new

Do not include a live capability URL or sensitive filename in a public issue.
