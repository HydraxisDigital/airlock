#!/usr/bin/env bash
set -euo pipefail
echo "Configuration Solidity / Foundry..."

echo "  ✔ forge $(forge --version)"

# No project is initialised here: bootstrap your app in the empty project/ dir,
# e.g. `cd project && forge init --no-git .`
