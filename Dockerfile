# Сборка
FROM ubuntu:24.04 AS builder

ENV DEBIAN_FRONTEND=noninteractive

# Устанавливаем зависимости и скачиваем установщик Rust
RUN apt-get update && apt-get install -y \
    build-essential \
    protobuf-compiler \
    libprotobuf-dev \
    pkg-config \
    libssl-dev \
    curl \
    && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /usr/src/app

# Копируем конфигурацию и исходники
COPY Cargo.toml Cargo.lock* ./
COPY build.rs ./
COPY proto ./proto
COPY src ./src

# Компилируем проект
RUN cargo build --release

# Runtime
FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y \
    ca-certificates \
    openssl \
    docker.io \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /usr/src/app/target/release/rdm-vision /usr/local/bin/rdm-vision

RUN mkdir -p /app/models

CMD ["rdm-vision"]
