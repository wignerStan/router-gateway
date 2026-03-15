FROM rust:1.85 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p gateway

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/* && \
    groupadd --system --gid 1001 gateway && \
    useradd --system --uid 1001 --gid gateway gateway

COPY --from=builder /app/target/release/gateway /usr/local/bin/
COPY --from=builder /app/deploy/entrypoint.sh /entrypoint.sh
COPY --from=builder /app/deploy/gateway.yaml.example /etc/gateway/gateway.yaml

RUN chmod +x /entrypoint.sh

USER gateway

EXPOSE 3000
ENTRYPOINT ["/entrypoint.sh"]
