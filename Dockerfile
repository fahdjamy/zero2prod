# We use the latest Rust stable release as base image
FROM rust:1.80.1

# Switch the working directory to `app` (equivalent to `cd app`)
# The `app` folder will be created for us by Docker in case it does not
# exist already.
WORKDIR /app

# Copy dependency files first for better caching
COPY Cargo.toml Cargo.lock ./

# Install the required system dependencies for our linking configuration
RUN apt update && apt install lld clang -y

# Copy all files from our working environment to our Docker image
COPY . .

ENV SQLX_OFFLINE=true

# Build the binary!
# Use the release profile to make it faaaast
RUN cargo build --release

ENV APP_ENVIRONMENT=production

# When `docker run` is executed, launch the binary!
ENTRYPOINT ["./target/release/zero2prod"]
