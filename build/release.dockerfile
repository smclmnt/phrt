FROM docker.io/rust:bullseye AS build
WORKDIR /usr/app
#RUN apk upgrade --update-cache --available
#RUN apk add musl-dev openssl openssl-dev  alpine-sdk pkgconf perl
COPY ./src ./src
COPY ./crates ./crates
COPY ./Cargo.toml ./Cargo.toml
RUN cargo build

FROM docker.io/debian:bullseye-slim AS release
WORKDIR /usr/phrt
RUN apt-get update && apt-get install -y ca-certificates --no-install-recommends && rm -rf /var/lib/apt/lists/*
COPY --from=build /usr/app/target/release/phrt /usr/phrt/phrt
COPY ./templates /usr/phrt/templates
COPY ./assets /usr/phrt/assets
ENTRYPOINT ["/usr/phrt/phrt", "--no-ansi", "--templates", "/usr/phrt/templates", "--asset-dir", "/usr/phrt/assets"]
#CMD ["ls", "-laFR", "/usr/phrt"]