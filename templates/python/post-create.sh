#!/usr/bin/env bash
set -euo pipefail
echo "Configuration Python / uv..."

echo "  ✔ uv $(uv --version)"

if [[ ! -f pyproject.toml ]]; then
    uv init --name "${PROJECT_NAME:-$(basename "$PWD")}"
    echo "  ✔ pyproject.toml created"
fi
