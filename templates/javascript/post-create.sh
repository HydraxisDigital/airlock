#!/usr/bin/env bash
set -euo pipefail
echo "Configuration JavaScript / pnpm..."

echo "  ✔ pnpm ignore-scripts = $(pnpm config get ignore-scripts)"
echo "  ✔ pnpm save-exact     = $(pnpm config get save-exact)"

# No project is initialised here: bootstrap your app in the empty project/ dir,
# e.g. `cd project && pnpm create vite@latest . --yes`
