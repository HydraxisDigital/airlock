#!/usr/bin/env bash
set -euo pipefail
echo "Configuration Python / uv..."

echo "  ✔ uv $(uv --version)"

# No project is initialised here: bootstrap your app in the empty project/ dir,
# e.g. `cd project && uv init`
