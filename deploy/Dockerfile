# Dedicated FERRITE server. Built from the workspace root:
#   docker build -f deploy/Dockerfile .
FROM rust:1-slim-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p ferrite-server

FROM debian:bookworm-slim
COPY --from=build /src/target/release/ferrite-server /usr/local/bin/ferrite-server
# Player records live here; the compose file mounts a volume on it.
RUN mkdir /data && chown nobody:nogroup /data
USER nobody
# 27500/udp game traffic, 27499/udp LAN discovery (multicast + broadcast).
EXPOSE 27500/udp 27499/udp
ENTRYPOINT ["ferrite-server"]
CMD ["--port", "27500", "--bots", "12"]
