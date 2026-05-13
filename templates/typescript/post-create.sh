#!/usr/bin/env bash
set -euo pipefail
echo "Configuration TypeScript / pnpm..."

echo "  ✔ pnpm ignore-scripts = $(pnpm config get ignore-scripts)"
echo "  ✔ pnpm save-exact     = $(pnpm config get save-exact)"

if [[ ! -f package.json ]]; then
    pnpm init
    # empty onlyBuiltDependencies: compliant with ignore-scripts, avoids pnpm 10+ warnings
    node -e "
const fs = require('fs');
const pkg = JSON.parse(fs.readFileSync('package.json', 'utf8'));
pkg.name = process.env.PROJECT_NAME || pkg.name;
pkg.pnpm = { onlyBuiltDependencies: [] };
fs.writeFileSync('package.json', JSON.stringify(pkg, null, 2) + '\n');
"
    echo "  ✔ package.json created"
fi
