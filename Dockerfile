ARG SKIP_BUILD=false

# Build stage
FROM rust:1-slim-bookworm as build

RUN apt-get update && apt-get install -y pkg-config libssl-dev && apt-get clean

WORKDIR /usr/src/admarus
COPY . .

ARG SKIP_BUILD
RUN if [ "$SKIP_BUILD" = "false" ]; then \
    cargo build --release --package admarusd; \
    fi

RUN cp target/release/admarusd /usr/local/bin/admarusd && \
    cd .. && \
    rm -rf admarus

# Final stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y libssl3 ca-certificates && apt-get clean

COPY --from=build /usr/local/bin/admarusd /usr/local/bin/admarusd

EXPOSE 4002
EXPOSE 5002

ENTRYPOINT ["/usr/local/bin/admarusd"]
