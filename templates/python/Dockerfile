FROM mcr.microsoft.com/devcontainers/python:3.12

# uv via image officielle Astral (multi-arch, pas de curl|sh)
COPY --from=ghcr.io/astral-sh/uv:latest /uv /uvx /usr/local/bin/

USER vscode
WORKDIR /workspace
