FROM rust:alpine AS test-base
RUN apk add musl-dev
RUN cargo install cargo-watch
RUN cargo install cargo-nextest

FROM test-base as test
WORKDIR /usr/phrt
COPY ./crates /usr/phrt/crates
COPY ./src /usr/phrt/src
COPY ./Cargo.toml /usr/phrt/Cargo.toml
ENTRYPOINT [ "cargo", "watch", "-x", "nextest r --workspace --no-fail-fast" ]