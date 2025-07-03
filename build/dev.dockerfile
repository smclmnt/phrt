FROM rust:alpine AS base
RUN apk add musl-dev
RUN cargo install cargo-watch
RUN cargo install cargo-nextest

FROM base AS development
WORKDIR /usr/phrt
COPY ./src /usr/phrt//src
COPY ./templates /usr/phrt/templates
COPY ./crates /usr/phrt/crates
COPY ./assets /usr/phrt/assets
COPY ./Cargo.toml /usr/phrt/Cargo.toml
RUN cargo test --workspace
CMD [ "cargo", "watch", "-x", "run", "--"]
