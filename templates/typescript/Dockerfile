FROM mcr.microsoft.com/devcontainers/typescript-node:__NODE_VERSION__

RUN apt-get update && apt-get install -y --no-install-recommends \
    zsh \
    && rm -rf /var/lib/apt/lists/*

# pnpm via corepack (include in Node 20)
RUN corepack enable && corepack prepare pnpm@latest --activate

# Security: ignore-scripts blocks the execution of third-party install scripts
RUN pnpm config set ignore-scripts true \
    && pnpm config set save-exact true

RUN chsh -s /usr/bin/zsh node

USER node
RUN if [ ! -f ~/.oh-my-zsh/oh-my-zsh.sh ]; then \
        rm -rf ~/.oh-my-zsh \
        && git clone --depth=1 https://github.com/ohmyzsh/ohmyzsh.git ~/.oh-my-zsh; \
    fi \
    && if [ ! -f ~/.zshrc ]; then \
        cp ~/.oh-my-zsh/templates/zshrc.zsh-template ~/.zshrc; \
    fi

WORKDIR /workspace
