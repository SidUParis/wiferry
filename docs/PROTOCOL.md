# Wiferry guest protocol 0.2-draft

This is the browser-facing contract implemented by the Rust alpha. It is a
draft, not a stable compatibility promise.

## Discovery and capability

The host displays:

```text
http://HOST:PORT/s/TOKEN/
```

`TOKEN` is 24 random bytes from the operating-system CSPRNG encoded as unpadded
base64url (192 bits). Possession grants access until expiry or revocation. The
token is never an encryption key.

The host accepts guest routes only when the transport peer address belongs to a
subnet assigned to a local interface (or is loopback). Proxy forwarding headers
are ignored.

## Session manifest

```http
GET /api/session/TOKEN
```

Relevant JSON fields:

```json
{
  "active": true,
  "deviceName": "Example laptop",
  "files": [
    {
      "id": "base64url-id",
      "name": "report.pdf",
      "size": 12345,
      "sizeLabel": "12 KB",
      "mime": "application/pdf"
    }
  ],
  "secondsRemaining": 1200,
  "canDownload": true
}
```

Responses use `404` for an unknown/rotated token and `410` for a known session
that has ended.

## File download

```http
GET  /api/session/TOKEN/files/FILE_ID
HEAD /api/session/TOKEN/files/FILE_ID
```

The alpha supports exactly one RFC-style byte range:

```http
Range: bytes=START-END
Range: bytes=START-
Range: bytes=-SUFFIX_LENGTH
```

It returns `200`, `206`, or `416` with `Accept-Ranges`, `Content-Length`,
`Content-Range` where applicable, MIME type, RFC 5987 filename, and a weak ETag
derived from the session file ID, size, and observed modification time.
Multi-range requests are rejected. A changed file size is rejected with `409`
instead of sending a misleading length.

The Rust data plane reads at most 128 KiB per iteration and rechecks the token,
generation, active state, and expiry before every chunk. Stop, rotation, or
expiry therefore closes an in-flight response at a chunk boundary.

## Management boundary

Management uses a separate listener bound to `127.0.0.1` and is never routed by
the LAN listener. At launch, a separate random capability is placed after `#` in
the local management URL, so it is not transmitted in an HTTP request. The page
stores it for that browser tab, removes it from the visible URL, and sends it as
`X-Wiferry-Admin` on every management API request. The server also requires a
allowlisted loopback `Host` authority and rejects foreign `Origin` values on
mutations. No HTML response embeds the capability. Guest clients cannot add
paths, stop sharing, rotate a token, or change the advertised interface.

## Planned resume contract

Protocol 0.3 will define a strong content ETag, `If-Range`, restart persistence,
and interruption test vectors. Until then, 0.2 provides standards-based Range
requests but does not advertise durable resume across host restarts.
