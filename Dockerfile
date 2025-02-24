##############################
## Build Rust backend
##############################
FROM rust:latest AS rust-builder

RUN update-ca-certificates

RUN apt-get update && apt-get install -y libclang-dev

WORKDIR /ebills

# start - build dependency cache
COPY Cargo.toml Cargo.lock ./

RUN mkdir -p /ebills/crates/bcr-ebill-core
RUN mkdir -p /ebills/crates/bcr-ebill-persistence
RUN mkdir -p /ebills/crates/bcr-ebill-api
RUN mkdir -p /ebills/crates/bcr-ebill-web

COPY crates/bcr-ebill-core/Cargo.toml ./crates/bcr-ebill-core/
COPY crates/bcr-ebill-persistence/Cargo.toml ./crates/bcr-ebill-persistence/
COPY crates/bcr-ebill-api/Cargo.toml ./crates/bcr-ebill-api/
COPY crates/bcr-ebill-web/Cargo.toml ./crates/bcr-ebill-web/

RUN mkdir ./crates/bcr-ebill-core/src && echo "fn main() {}" > ./crates/bcr-ebill-core/src/lib.rs
RUN mkdir ./crates/bcr-ebill-persistence/src && echo "fn main() {}" > ./crates/bcr-ebill-persistence/src/lib.rs
RUN mkdir ./crates/bcr-ebill-api/src && echo "fn main() {}" > ./crates/bcr-ebill-api/src/lib.rs
RUN mkdir ./crates/bcr-ebill-web/src && echo "fn main() {}" > ./crates/bcr-ebill-web/src/main.rs

# Build dependencies (without compiling main source files)
RUN cargo build --release
# end - build dependency cache

COPY ./ .

# need to break the cargo cache
RUN touch -a -m ./crates/bcr-ebill-web/src/main.rs
RUN cargo build --release --features embedded-db

##############################
## Create image
##############################
FROM ubuntu:22.04

RUN apt-get update && \
  apt-get install -y ca-certificates && \
  apt-get clean

WORKDIR /ebills

# Copy essential build files
COPY --from=rust-builder /ebills/target/release/bcr-ebill-web ./bitcredit
COPY --from=rust-builder /ebills/frontend ./frontend

# Create additional directories and set user permissions
RUN mkdir data

ENV ROCKET_ADDRESS=0.0.0.0

# Expose web server port
EXPOSE 8000

# Expose P2P port
EXPOSE 1908

CMD ["/ebills/bitcredit"]
