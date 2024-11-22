##############################
## Build Rust backend
##############################
FROM node:16-alpine AS frontend-builder

WORKDIR /frontend

COPY ./frontend .

RUN npm install --legacy-peer-deps && npm run build

##############################
## Build Rust backend
##############################
FROM rust:latest AS rust-builder

RUN update-ca-certificates

RUN apt-get update && apt-get install -y protobuf-compiler

# Create appuser
ENV USER=ebills
ENV UID=10001

RUN adduser \
  --disabled-password \
  --gecos "" \
  --home "/nonexistent" \
  --shell "/sbin/nologin" \
  --no-create-home \
  --uid "${UID}" \
  "${USER}"


WORKDIR /ebills

COPY ./ .

COPY --from=frontend-builder /frontend_build ./frontend_build

RUN cargo build --release --features embedded-db

##############################
## Create image
##############################
FROM ubuntu:22.04

RUN apt-get update && \
  apt-get install -y ca-certificates && \
  apt-get clean

# Import user and group files from builder.
COPY --from=rust-builder /etc/passwd /etc/passwd
COPY --from=rust-builder /etc/group /etc/group

WORKDIR /ebills

# Copy essential build files
COPY --from=rust-builder /ebills/target/release/bitcredit ./bitcredit
COPY --from=rust-builder /ebills/target/release/frontend_build ./frontend_build
COPY --from=rust-builder /ebills/target/release/bootstrap ./bootstrap

# Create additional directories and set user permissions
RUN mkdir identity bills bills_keys contacts quotes && chown -R ebills:ebills /ebills

# Use unprivileged user.
USER ebills:ebills

ENV ROCKET_ADDRESS=0.0.0.0

# Expose web server port
EXPOSE 8000

# Expose P2P port
EXPOSE 1908

CMD ["/ebills/bitcredit"]
