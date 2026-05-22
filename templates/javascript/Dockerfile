FROM mcr.microsoft.com/devcontainers/javascript-node:__NODE_VERSION__

# pnpm via corepack (include in Node 20)
RUN corepack enable && corepack prepare pnpm@latest --activate

# Security : ignore-scripts blocks the execution of install scripts
RUN pnpm config set ignore-scripts true \
    && pnpm config set save-exact true

USER node
WORKDIR /workspace
