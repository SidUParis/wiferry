# Releasing Wiferry

Wiferry `0.2.0-alpha.1` is published only by the `Native bundles` GitHub Actions
workflow. Do not upload locally built binaries to a public release.

## Release gate

1. Update the version in `Cargo.toml` and regenerate `Cargo.lock` if required.
2. Ensure CI passes on Linux x86-64, Windows x86-64, macOS arm64, and macOS
   x86-64.
3. Create a tag whose exact value is `v` followed by the Cargo package version.
   For this release, the only accepted tag is `v0.2.0-alpha.1`.
4. The release workflow rebuilds and tests the frontend and Rust host on each
   native runner. Its smoke test shares a filename containing spaces and Unicode,
   then verifies SHA-256 for both a full download and a cross-chunk HTTP Range.
5. The publish job requires exactly four archives and their four checksum files
   and verifies every checksum again.
6. Because immutable releases are enabled, the workflow creates a draft,
   uploads all eight assets, and only then publishes it as a prerelease. GitHub
   locks the published tag and assets and creates a release attestation.

## Expected assets

- `wiferry-0.2.0-alpha.1-linux-x86_64.tar.gz`
- `wiferry-0.2.0-alpha.1-windows-x86_64.zip`
- `wiferry-0.2.0-alpha.1-macos-arm64.tar.gz`
- `wiferry-0.2.0-alpha.1-macos-x86_64.tar.gz`
- one adjacent `.sha256` file for each archive

Linux and macOS archives contain `wiferry`; the Windows archive contains
`wiferry.exe`. Each archive also contains the English and Chinese READMEs,
changelog, license, and security policy.

Signing and notarization are not part of this alpha release. Do not describe an
artifact as verified for a platform until its public native runner job passes and
the downloaded archive independently passes its recorded SHA-256 check.
