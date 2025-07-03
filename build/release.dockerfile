FROM rust:alpine AS build
WORKDIR /usr/app
RUN apk add musl-dev
COPY ./src ./src
COPY ./Cargo.toml ./Cargo.toml
RUN cargo test --workspace
RUN cargo build --release

FROM alpine AS release
COPY --from=build /usr/app/target/release/phrt /usr/phrt/phrt
COPY ./templates /usr/phrt/templates
COPY ./assets /usr/phrt/assets
CMD ["/usr/phrt/phrt", "--no-ansi", "--templates", "/usr/phrt/templates", "--asset-dir", "/usr/phrt/assets"]