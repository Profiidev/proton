ARG TARGET=x86_64-unknown-linux-musl

FROM ghcr.io/profiidev/images/rust-musl-builder:main AS planner

COPY backend/Cargo.toml ./Cargo.lock ./

RUN cargo chef prepare --recipe-path recipe.json --bin proton-backend

FROM ghcr.io/profiidev/images/rust-musl-builder:main AS builder

ARG TARGET
ENV TARGET=$TARGET

COPY --from=planner /app/recipe.json .

RUN cargo chef cook --release --target $TARGET

COPY backend/src ./src
COPY backend/Cargo.toml ./Cargo.lock ./

RUN cargo build --release --target $TARGET --bin proton-backend
RUN mv ./target/$TARGET/release/proton-backend ./app

FROM alpine

COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

WORKDIR /app
COPY --from=builder /app/app /usr/local/bin/

CMD ["app"]