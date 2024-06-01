ARG SKIP_BUILD=false

# Build stage
FROM messense/rust-musl-cross:x86_64-musl as build

RUN apt-get update && apt-get install -y pkg-config libssl-dev && apt-get clean

WORKDIR /usr/src/admarus
COPY . .

ARG SKIP_BUILD
RUN if [ "$SKIP_BUILD" = "false" ]; then \
        cargo build --release --target=x86_64-unknown-linux-musl --package admarusd; \
    fi;

RUN cp target/x86_64-unknown-linux-musl/release/admarusd /usr/local/bin/admarusd && \
    cd .. && \
    rm -rf admarus

# Final stage
FROM alpine:latest

COPY --from=build /usr/local/bin/admarusd /usr/local/bin/admarusd

EXPOSE 4002
EXPOSE 5002

CMD ["/usr/local/bin/admarusd"]
