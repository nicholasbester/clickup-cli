# Build stage
FROM rust:1.87-slim AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ src/

RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/clickup /usr/local/bin/clickup

ENTRYPOINT ["clickup"]
CMD ["mcp", "serve"]
