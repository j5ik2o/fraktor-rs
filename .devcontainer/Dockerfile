FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

# Install additional tools
RUN cargo install cargo-make cargo-dylint dylint-link --locked

# Install useful development tools
RUN apt-get update && apt-get install -y \
    git \
    curl \
    && rm -rf /var/lib/apt/lists/*
