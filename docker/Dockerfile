# The image the home lab's Jenkins runs splash's checks in. The official Rust
# image installs the minimal rustup profile, which leaves out rustfmt and
# clippy, so they are added here along with cargo-tarpaulin for coverage.
FROM rust:1.99.1-trixie

ARG TARPAULIN_VERSION=0.37.5

RUN rustup component add rustfmt clippy \
    && cargo install --locked cargo-tarpaulin --version "$TARPAULIN_VERSION" \
    && rm -rf "$CARGO_HOME/registry" "$CARGO_HOME/git"

WORKDIR /workspace
