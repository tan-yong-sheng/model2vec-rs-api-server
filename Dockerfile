# Build stage
FROM rust:1.83-alpine AS builder

WORKDIR /app

# Install build dependencies
RUN apk add --no-cache musl-dev openssl-dev clang-dev gcc g++ libc-dev git

# Copy source files
COPY Cargo.toml .
COPY Cargo.lock .
COPY src/ src/

# Build the binary
RUN cargo build --release --target x86_64-unknown-linux-musl

# Strip binary for smaller size
RUN strip target/x86_64-unknown-linux-musl/release/model2vec-api

# Download model files from HuggingFace using git
FROM alpine:3.19 AS model_downloader
ARG MODEL_NAME=minishlab/potion-base-8M
RUN apk add --no-cache git

# Clone the model repo (shallow clone for faster download)
RUN mkdir -p /app/models && \
    git clone --depth 1 --branch main https://huggingface.co/${MODEL_NAME} /tmp/model_repo && \
    cp -r /tmp/model_repo/* /app/models/ && \
    rm -rf /tmp/model_repo && \
    ls -la /app/models/

# Runtime stage
FROM alpine:3.19 AS runtime

WORKDIR /app

# Install runtime dependencies
RUN apk add --no-cache openssl ca-certificates

# Copy model files
COPY --from=model_downloader /app/models /app/models

# Copy binary from builder
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/model2vec-api /app/model2vec-api

# Set environment
ENV MODEL_NAME=minishlab/potion-base-8M
ENV ALIAS_MODEL_NAME=minishlab-potion-base-8M
ENV PORT=8080
ENV RUST_LOG=info

EXPOSE 8080

ENTRYPOINT ["/app/model2vec-api"]
