# Dockerfile for a2b_news_aggregator
FROM rust:1.85 AS builder
WORKDIR /usr/src/a2b_news_aggregator
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main(){}" > src/main.rs
RUN cargo build --release --locked
RUN rm -rf src
COPY src ./src
RUN cargo build --release --locked
RUN echo "--- Linked shared libraries for a2b_news_aggregator ---" && \
    ldd /usr/src/a2b_news_aggregator/target/release/a2b_news_aggregator && \
    echo "--------------------------------------------------------"
FROM debian:bookworm AS runtime
RUN apt-get update && \
    apt-get install -y libssl3 ca-certificates && \
    rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /usr/src/a2b_news_aggregator/target/release/a2b_news_aggregator .
RUN groupadd -r appgroup && useradd -r -g appgroup appuser
USER appuser
EXPOSE 3030
ENV RUST_LOG="info,a2b_news_aggregator=debug"
CMD ["./a2b_news_aggregator"]