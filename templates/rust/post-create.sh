#!/usr/bin/env bash
set -euo pipefail
echo "Configuration Rust..."

echo "  ✔ cargo-audit $(cargo audit --version 2>/dev/null || echo 'not installed')"
echo "  ✔ cargo-deny  $(cargo deny --version 2>/dev/null || echo 'not installed')"

if [[ ! -f Cargo.toml ]]; then
    cargo init --name "${PROJECT_NAME:-$(basename "$PWD")}" .
    echo "  ✔ Cargo.toml created"
fi
