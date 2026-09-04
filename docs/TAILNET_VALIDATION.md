# Tailnet validation

Date: 2026-09-04

This is the first real second-device validation of Wiferry's explicit
`tailscale` transport. Addresses, hostnames, capability tokens, and filenames
are intentionally omitted.

## Environment

- Wiferry host: Ubuntu Linux x86-64
- Guest: a second online Linux device in the same tailnet
- Tailscale interface: IPv4 `/32`
- Observed Tailscale path: direct UDP, 5 ms during the test
- Payload: 13.4 MiB MP3

## Results

| Check | Result |
|---|---|
| Host detects its Tailscale IPv4 | Pass |
| Guest manifest reports `transport: tailscale` | Pass |
| Full remote download SHA-256 equals source | Pass |
| Cross-128-KiB Range SHA-256 equals source slice | Pass |
| Tailnet policy through Tailnet address | HTTP 200 |
| Tailnet policy through host LAN address | HTTP 403 |
| LAN policy through Tailnet address | HTTP 403 |
| LAN policy through selected LAN address | HTTP 200 |

The test establishes address discovery, real remote transfer, and
transport-scoped admission on this tailnet. It does not claim that every
Tailscale ACL, DERP path, mobile client, or network-change scenario has been
tested.
