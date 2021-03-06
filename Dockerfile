# Stage 0: builder image
FROM rust:latest as builder

ENV CROSS_MUSL_COMMIT="850fbd1024800a88d679a2891a03662dcec276f5"

RUN curl -fsSLO "https://github.com/japaric/cross/raw/$CROSS_MUSL_COMMIT/docker/musl.sh" \
    && curl -fsSLO "https://github.com/japaric/cross/raw/$CROSS_MUSL_COMMIT/docker/openssl.sh" \
    && curl -fsSL -o /usr/local/bin/musl-gcc "https://github.com/japaric/cross/raw/$CROSS_MUSL_COMMIT/docker/musl-gcc.x86_64-unknown-linux-musl" \
    && curl -fsSL -o /usr/local/lib/musl-gcc.specs "https://github.com/japaric/cross/raw/$CROSS_MUSL_COMMIT/docker/musl-gcc.specs.x86_64-unknown-linux-musl" \
    && bash musl.sh 1.1.15 \
    && bash openssl.sh linux-x86_64 musl- -static \
    && rustup target add x86_64-unknown-linux-musl

ENV CC_x86_64_unknown_linux_musl=musl-gcc \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib

RUN mkdir /app
WORKDIR /app

COPY . /app

RUN set -ex ;\
    cargo build --target x86_64-unknown-linux-musl --release ;\
    cargo test --target x86_64-unknown-linux-musl --release -- --nocapture ;\
    :

# Stage 1: final image
FROM alpine

RUN apk add --no-cache nftables

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/dfw /dfw
ENTRYPOINT ["/dfw"]

