# syntax=docker/dockerfile:1
FROM rust:bookworm AS builder
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY examples examples
COPY apps apps
RUN cargo build --release -p qefro-cli

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /src/target/release/qefro /usr/local/bin/qefro
WORKDIR /app
COPY apps apps
ENV QEFRO_BIND=0.0.0.0:8080
EXPOSE 8080
CMD ["qefro", "serve"]
