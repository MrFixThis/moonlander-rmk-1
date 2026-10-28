FROM rust:1-slim-bookworm

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libudev-dev \
    libusb-1.0-0-dev \
    dfu-util \
    gcc-arm-none-eabi \
    binutils-arm-none-eabi \
    ca-certificates \
    git \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add thumbv7em-none-eabihf \
    && rustup component add rust-src

RUN cargo install --locked flip-link

WORKDIR /workspace

CMD ["bash"]
