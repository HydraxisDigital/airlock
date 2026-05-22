FROM mcr.microsoft.com/devcontainers/rust:__RUST_VERSION__

RUN cargo install cargo-audit cargo-deny \
    && chown -R vscode:vscode "$CARGO_HOME"

USER vscode
WORKDIR /workspace
