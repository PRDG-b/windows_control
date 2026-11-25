FROM rust:1.90-bookworm AS rust-toolchain
FROM mcr.microsoft.com/playwright/python:v1.64.1-noble

COPY --from=rust-toolchain /usr/local/cargo /usr/local/cargo
COPY --from=rust-toolchain /usr/local/rustup /usr/local/rustup
ENV CARGO_HOME=/usr/local/cargo RUSTUP_HOME=/usr/local/rustup
ENV PATH="/usr/local/cargo/bin:${PATH}"
RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential tesseract-ocr tesseract-ocr-eng \
    && rm -rf /var/lib/apt/lists/* \
    && python3 -m pip install --no-cache-dir playwright==1.64.1 rerun-sdk==0.43.2
WORKDIR /workspace
