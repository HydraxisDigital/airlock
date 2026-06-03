FROM mcr.microsoft.com/devcontainers/base:ubuntu-22.04

RUN apt-get update && apt-get install -y --no-install-recommends \
    zsh \
    && rm -rf /var/lib/apt/lists/*

# Foundry — copied from official image (no curl | sh)
COPY --from=ghcr.io/foundry-rs/foundry:latest /usr/local/bin/forge  /usr/local/bin/forge
COPY --from=ghcr.io/foundry-rs/foundry:latest /usr/local/bin/anvil  /usr/local/bin/anvil
COPY --from=ghcr.io/foundry-rs/foundry:latest /usr/local/bin/cast   /usr/local/bin/cast
COPY --from=ghcr.io/foundry-rs/foundry:latest /usr/local/bin/chisel /usr/local/bin/chisel

RUN chsh -s /usr/bin/zsh vscode

USER vscode
RUN if [ ! -f ~/.oh-my-zsh/oh-my-zsh.sh ]; then \
        rm -rf ~/.oh-my-zsh \
        && git clone --depth=1 https://github.com/ohmyzsh/ohmyzsh.git ~/.oh-my-zsh; \
    fi \
    && if [ ! -f ~/.zshrc ]; then \
        cp ~/.oh-my-zsh/templates/zshrc.zsh-template ~/.zshrc; \
    fi

WORKDIR /workspace
