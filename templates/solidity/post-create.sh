#!/usr/bin/env bash
set -euo pipefail
echo "Configuration Solidity / Foundry..."

echo "  ✔ forge $(forge --version)"

if [[ ! -f foundry.toml ]]; then
    forge init --no-git --no-commit .
    echo "  ✔ Foundry project initialised"
fi
