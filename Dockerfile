# ------------------------------------------------------------------------------
# Cargo Build Stage
# ------------------------------------------------------------------------------

FROM rust:1.46.0 AS builder

WORKDIR /app

COPY Cargo.lock Cargo.lock
COPY Cargo.toml Cargo.toml

RUN mkdir src/
RUN mkdir src/bin
RUN touch src/lib.rs
RUN echo "fn main() {println!(\"if you see this, the build broke\")}" > src/bin/candidate_verifier.rs

RUN cargo build --release

RUN rm -f target/release/deps/*candidate-verifier*
RUN rm -rf src

COPY . .

RUN cargo build --release

# # ------------------------------------------------------------------------------
# # Final Stage
# # ------------------------------------------------------------------------------

FROM debian:buster-slim

RUN apt-get update && apt-get install -y libssl-dev ca-certificates
RUN update-ca-certificates --fresh

WORKDIR /app

RUN mkdir config

COPY --from=builder /app/target/release/candidate-verifier .
COPY config/service.yml .
COPY config/kusama_candidates.yml .
COPY config/polkadot_candidates.yml .

CMD ["./candidate-verifier"]
