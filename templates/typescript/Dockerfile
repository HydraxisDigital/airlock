FROM mcr.microsoft.com/devcontainers/javascript-node:20

# pnpm via corepack (inclus dans Node 20)
RUN corepack enable && corepack prepare pnpm@latest --activate

# Sécurité : ignore-scripts bloque l'exécution de scripts d'install tiers
RUN pnpm config set ignore-scripts true \
    && pnpm config set save-exact true

USER node
WORKDIR /workspace
