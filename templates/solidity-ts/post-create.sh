#!/usr/bin/env bash
set -euo pipefail
echo "Configuration Solidity + TypeScript..."

echo "  ✔ forge $(forge --version)"
echo "  ✔ pnpm ignore-scripts = $(pnpm config get ignore-scripts)"

# No project is initialised here: bootstrap your app in the empty project/ dir,
# e.g. `cd project && forge init --no-git .` or `pnpm create hardhat`
