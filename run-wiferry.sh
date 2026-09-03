#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "$0")"
if [[ ! -x .runtime-venv/bin/python ]]; then
  python3 -m venv .runtime-venv
fi
.runtime-venv/bin/pip install --quiet --upgrade .
exec .runtime-venv/bin/wiferry "$@"
