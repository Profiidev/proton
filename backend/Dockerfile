ARG TARGET=x86_64-unknown-linux-musl

FROM ghcr.io/profiidev/images/rust-musl-builder:main@sha256:b7a21083e422aa214693c2ad48761a38b6ad55b064136ab3e387e17752e4f43d AS planner

COPY backend/Cargo.toml ./Cargo.lock ./

RUN cargo chef prepare --recipe-path recipe.json --bin proton-backend

FROM ghcr.io/profiidev/images/rust-musl-builder:main@sha256:b7a21083e422aa214693c2ad48761a38b6ad55b064136ab3e387e17752e4f43d AS builder

ARG TARGET
ENV TARGET=$TARGET

COPY --from=planner /app/recipe.json .

RUN cargo chef cook --release --target $TARGET

COPY backend/src ./src
COPY backend/Cargo.toml ./Cargo.lock ./

RUN cargo build --release --target $TARGET --bin proton-backend
RUN mv ./target/$TARGET/release/proton-backend ./app

FROM alpine@sha256:5b10f432ef3da1b8d4c7eb6c487f2f5a8f096bc91145e68878dd4a5019afde11

COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

WORKDIR /app
COPY --from=builder /app/app /usr/local/bin/

CMD ["app"]