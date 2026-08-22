FROM rust:1.98.0-slim-bookworm AS builder
WORKDIR /backend
COPY ./backend .
RUN apt-get update && apt-get install -y libssl-dev pkg-config
RUN cargo build --release

FROM debian:bookworm-slim AS runner
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
RUN groupadd -r group && useradd -r -g group user

WORKDIR /app
COPY ./frontend ./frontend
COPY ./backend/templates ./backend/templates
RUN mkdir -p backend
COPY --from=builder --chown=user:group /backend/target/release/wip-server ./backend/wip-server
RUN chmod +x ./backend/wip-server
RUN chown -R user:group /app/backend

WORKDIR /app/backend
USER user
ENTRYPOINT ["./wip-server"]