#!/usr/bin/env bash
set -euo pipefail
echo "Configuration Rust..."

echo "  ✔ cargo-audit $(cargo audit --version 2>/dev/null || echo 'not installed')"
echo "  ✔ cargo-deny  $(cargo deny --version 2>/dev/null || echo 'not installed')"

# No project is initialised here: bootstrap your app in the empty project/ dir,
# e.g. `cd project && cargo init .`
