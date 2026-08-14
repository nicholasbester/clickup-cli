# Container image for the CLI / MCP server (default CMD runs `mcp serve`,
# JSON-RPC over stdio). MCP hosting platforms (e.g. Glama) build from this
# file in preference to an inferred build spec.
#
# Run locally:
#   docker build -t clickup-cli .
#   docker run -i -e CLICKUP_TOKEN=pk_... -e CLICKUP_WORKSPACE=123 clickup-cli
#   docker run -e CLICKUP_TOKEN=pk_... clickup-cli task list --list 123

# Build stage
# Major-pinned so the toolchain tracks stable and stays >= the crate's
# rust-version (MSRV) in Cargo.toml. A minor pin (rust:1.87) silently fell
# below the MSRV once it was bumped to 1.88.
# Both stages MUST pin the same Debian suite (-trixie here): a builder on a
# newer suite links against a newer glibc than the runtime image provides
# (rust:1-slim on trixie/glibc 2.39 vs bookworm/glibc 2.36 fails at runtime).
# At the next Debian stable, bump both -trixie references together.
FROM rust:1-slim-trixie AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ src/

# The crate ships two identical binaries, `clickup-cli` and `clkup`
# (renamed from `clickup` in #41); build one and install it below under
# the historical image command name `clickup`.
RUN cargo build --release --bin clickup-cli

# Runtime stage
FROM debian:trixie-slim

# ca-certificates: rustls loads the system trust store for TLS to the ClickUp API.
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/clickup-cli /usr/local/bin/clickup
# Hardlink the documented binary names (docs/install.md: every install
# method ships `clickup-cli` and `clkup`); `clickup` stays the image's
# historical entrypoint name.
RUN ln /usr/local/bin/clickup /usr/local/bin/clickup-cli \
    && ln /usr/local/bin/clickup /usr/local/bin/clkup

ENTRYPOINT ["clickup"]
CMD ["mcp", "serve"]
