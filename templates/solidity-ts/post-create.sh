#!/usr/bin/env bash
set -euo pipefail
echo "Configuration Solidity + TypeScript..."

echo "  ✔ forge $(forge --version)"
echo "  ✔ pnpm ignore-scripts = $(pnpm config get ignore-scripts)"

if [[ ! -f foundry.toml ]]; then
    forge init --no-git --no-commit .
    echo "  ✔ Foundry project initialised"
fi

if [[ ! -f package.json ]]; then
    pnpm init
    node -e "
const fs = require('fs');
const pkg = JSON.parse(fs.readFileSync('package.json', 'utf8'));
pkg.name = process.env.PROJECT_NAME || pkg.name;
pkg.pnpm = { onlyBuiltDependencies: [] };
fs.writeFileSync('package.json', JSON.stringify(pkg, null, 2) + '\n');
"
    echo "  ✔ package.json created"
fi
