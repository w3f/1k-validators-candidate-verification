FROM rust

COPY . .

RUN cargo run --bin candidate-verifier
