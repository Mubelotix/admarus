FROM rust:1-slim-bookworm as build

RUN apt-get update && apt-get install -y pkg-config libssl-dev && apt-get clean

WORKDIR /usr/src/admarus
COPY . .

RUN cd daemon && \
    cargo build --release && \
    mv ../target/release/admarusd /usr/local/bin/admarusd && \
    cd ../../ && \
    rm -rf admarus

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y libssl ca-certificates && apt-get clean

COPY --from=build /usr/local/bin/admarusd /usr/local/bin/admarusd

EXPOSE 4002
EXPOSE 5002

ENTRYPOINT ["/usr/local/bin/admarusd"]
