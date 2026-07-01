FROM mcr.microsoft.com/devcontainers/rust:__RUST_VERSION__

RUN apt-get update && apt-get install -y --no-install-recommends \
    zsh \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-audit cargo-deny \
    && chown -R vscode:vscode "$CARGO_HOME"

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
