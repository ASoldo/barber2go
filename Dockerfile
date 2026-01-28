FROM rust:1.93-slim AS builder

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends libsqlite3-dev libssl-dev pkg-config ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY templates ./templates
COPY static ./static
COPY migrations ./migrations

RUN cargo build --release

FROM debian:trixie-slim

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends libsqlite3-0 libssl3 ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/barber2go /app/barber2go
COPY templates /app/templates
COPY static /app/static
COPY migrations /app/migrations

ENV PORT=8080
EXPOSE 8080

CMD ["/app/barber2go"]
