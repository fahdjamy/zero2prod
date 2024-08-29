# // https://github.com/LukeMathWalker/cargo-chef

FROM lukemathwalker/cargo-chef:latest-rust-1.80.1 AS chef
WORKDIR /app
RUN apt update && apt install lld clang -y

FROM chef AS planner
COPY . .
# Compute a lock-like file for our project
RUN cargo chef prepare  --recipe-path recipe.json

# Use the latest Rust stable release as base image
# Build stage
FROM chef AS builder

COPY --from=planner /app/recipe.json recipe.json
# Build our project dependencies, not our application!
RUN cargo chef cook --release --recipe-path recipe.json
# Up to this point, if our dependency tree stays the same,
# all layers should be cached.

# Copy all files from our working environment to our Docker image
COPY . .

ENV SQLX_OFFLINE=true

# Build the binary!
# Use the release profile to make it faaaast
RUN cargo build --release

# RUNTIME stage
# use the bare operating system as base image
FROM debian:bookworm-slim AS runtime

# Switch the working directory to `app` (equivalent to `cd app`)
# The `app` folder will be created for us by Docker in case it does not
# exist already.
WORKDIR /app

# Install OpenSSL - it is dynamically linked by some of our dependencies
# Install ca-certificates - it is needed to verify TLS certificates
# when establishing HTTPS connections
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends openssl ca-certificates \
    # Clean up
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*

# Copy the compiled binary from the builder environment
# to the runtime environment
COPY --from=builder /app/target/release/zero2prod zero2prod

# We need the configuration folder at runtime!
COPY configuration configuration

ENV APP_ENVIRONMENT=production

# When `docker run` is executed, launch the binary!
ENTRYPOINT ["./zero2prod"]
