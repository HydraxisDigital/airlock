
# Shell aliases
for rc_file in ~/.zshrc ~/.bashrc; do
    touch "$rc_file"
    echo "alias ll='ls -lh'" >> "$rc_file"
    cat >> "$rc_file" <<'AIRLOCK_APT_HELP'

airlock_apt_help() {
    cat <<'EOF'
airlock disables runtime apt/sudo inside the container.
Add system packages to .devcontainer/Dockerfile, before the USER line, then rebuild the container:

  RUN apt-get update && apt-get install -y --no-install-recommends <package> \
      && rm -rf /var/lib/apt/lists/*

In VS Code: Dev Containers: Rebuild Container
EOF
}

apt() {
    airlock_apt_help
    return 1
}

apt-get() {
    airlock_apt_help
    return 1
}
AIRLOCK_APT_HELP
done

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
