FROM mcr.microsoft.com/devcontainers/python:__PYTHON_VERSION__

# uv via official Astral image (multi-arch, no curl|sh)
COPY --from=ghcr.io/astral-sh/uv:latest /uv /uvx /usr/local/bin/

USER vscode
WORKDIR /workspace
