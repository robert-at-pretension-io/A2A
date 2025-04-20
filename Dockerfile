FROM rust:1.70-slim as builder

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./
COPY build.rs ./
COPY a2a_schema.config a2a_schema.json ./

# Create a dummy source file to build dependencies
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy actual source code
COPY src/ src/

# Install cargo-typify for schema type generation
RUN cargo install cargo-typify

# Generate types from schema
RUN if [ ! -f "src/types.rs" ]; then \
    cargo run -- config generate-types; \
    fi

# Build the application 
RUN cargo build --release

# Create a new stage with a minimal image
FROM debian:bullseye-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates libssl-dev && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary from the builder stage
COPY --from=builder /app/target/release/a2a-test-suite .

# Set the binary as the entrypoint
ENTRYPOINT ["/app/a2a-test-suite"]

# Default command (can be overridden)
CMD ["--help"]