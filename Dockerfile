FROM docker.io/library/rust:1.58-buster

COPY Cargo.lock /tmp/
COPY Cargo.toml /tmp/

WORKDIR /tmp/

RUN mkdir src
RUN touch src/main.rs

RUN cargo fetch
