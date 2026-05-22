FROM mcr.microsoft.com/devcontainers/javascript-node:__NODE_VERSION__

# Foundry — copied from official image (no curl | sh)
COPY --from=ghcr.io/foundry-rs/foundry:latest /usr/local/bin/forge  /usr/local/bin/forge
COPY --from=ghcr.io/foundry-rs/foundry:latest /usr/local/bin/anvil  /usr/local/bin/anvil
COPY --from=ghcr.io/foundry-rs/foundry:latest /usr/local/bin/cast   /usr/local/bin/cast
COPY --from=ghcr.io/foundry-rs/foundry:latest /usr/local/bin/chisel /usr/local/bin/chisel

# pnpm via corepack
RUN corepack enable && corepack prepare pnpm@latest --activate
RUN pnpm config set ignore-scripts true \
    && pnpm config set save-exact true

USER node
WORKDIR /workspace
