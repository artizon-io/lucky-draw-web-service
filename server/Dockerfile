FROM rust:latest

RUN apt-get update -y && \
  apt-get install -y pkg-config libssl-dev

RUN cargo install sqlx-cli

WORKDIR /usr/src/app

COPY . .

CMD sqlx migrate run && cargo run --release