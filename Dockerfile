FROM ubuntu:24.04 AS builder

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y \
    build-essential \
    nasm \
    pkg-config \
    libssl-dev \
    curl \
    && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /usr/src/app

COPY Cargo.toml Cargo.lock* ./
COPY build.rs ./
COPY proto ./proto
COPY src ./src

RUN cargo build --release

RUN mkdir -p /out && \
    cp target/release/rdm-vision /out/ && \
    (cp target/release/*.so* /out/ 2>/dev/null || true)

FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libstdc++6 \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -r -s /bin/false rdmv

WORKDIR /app

COPY --from=builder /out/ /app/
COPY models ./models

RUN chown -R rdmv:rdmv /app

USER rdmv

ENV LD_LIBRARY_PATH=/app

CMD ["/app/rdm-vision"]
