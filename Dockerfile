# Runnable entry for MCP registries and scanners: builds the cloud-sessions CLI and runs `cloud-sessions mcp` on stdio.
# No login, no token and no network are needed to start it or to list its tools; auth is checked only when a tool is called.
FROM rust:1-slim AS build
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p cloud-sessions-cli

FROM debian:stable-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/cloud-sessions /usr/local/bin/cloud-sessions
ENTRYPOINT ["cloud-sessions", "mcp"]
