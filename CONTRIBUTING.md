# Contributing to Wiferry

Thank you for helping build a small, auditable LAN file-delivery tool.

## Before opening a change

- Use an issue for a substantial feature or protocol change.
- Keep the receiver browser-only and the default path cloud-free.
- Do not describe a change as faster, lighter, private, encrypted, or resumable
  without a test or benchmark that establishes the exact scope.
- Report vulnerabilities through GitHub private vulnerability reporting, not a
  public issue.

## Development checks

```bash
npm --prefix frontend ci
npm --prefix frontend run lint
npm --prefix frontend run build
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
```

The Python reference contract currently remains active:

```bash
python -m venv .venv
.venv/bin/pip install -e '.[dev]'
.venv/bin/ruff check .
.venv/bin/pytest -q
```

## Pull requests

- Add focused tests for behavior changes.
- Update the threat model when a trust boundary changes.
- Include Linux, Windows, and macOS impact.
- Keep `Cargo.lock` and `frontend/package-lock.json` consistent.
- For performance work, commit raw results, environment details, checksum
  validation, and the runner—not just a summary number.
