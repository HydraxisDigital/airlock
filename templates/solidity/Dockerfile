FROM mcr.microsoft.com/devcontainers/base:ubuntu-22.04

# Foundry — copied from official image (no curl | sh)
COPY --from=ghcr.io/foundry-rs/foundry:latest /usr/local/bin/forge  /usr/local/bin/forge
COPY --from=ghcr.io/foundry-rs/foundry:latest /usr/local/bin/anvil  /usr/local/bin/anvil
COPY --from=ghcr.io/foundry-rs/foundry:latest /usr/local/bin/cast   /usr/local/bin/cast
COPY --from=ghcr.io/foundry-rs/foundry:latest /usr/local/bin/chisel /usr/local/bin/chisel

USER vscode
WORKDIR /workspace
