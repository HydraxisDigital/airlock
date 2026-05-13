
# Shell aliases
echo "alias ll='ls -lh'" >> ~/.bashrc

# Secrets symlink
if [[ -f /run/secrets/env ]]; then
    ln -sf /run/secrets/env /workspace/.env
    echo "  ✔ /workspace/.env → /run/secrets/env"
fi

# Isolation check
echo ""
echo "Isolation check:"

if [[ ! -d "/Users" ]]; then
    echo "  ✔ /Users inaccessible (host isolated)"
else
    echo "  ⚠ /Users visible — check mounts"
fi

found_sensitive=0
for var in AWS_SECRET SSH_AUTH_SOCK GITHUB_TOKEN WALLET PRIVATE_KEY MNEMONIC; do
    if [[ -n "${!var:-}" ]]; then
        echo "  ⚠ Sensitive variable detected: $var"
        found_sensitive=1
    fi
done
if [[ $found_sensitive -eq 0 ]]; then
    echo "  ✔ No sensitive variables detected"
fi

echo ""
echo "✔ Container ready. Happy coding!"
