FROM rust:1.89-trixie as builder
WORKDIR /usr/src/thumbnail-service-rs
RUN apt update && apt install -y libjpeg-dev gfortran libopenblas-dev
COPY . .
RUN cargo install --path .

FROM debian:trixie-slim
RUN apt update && apt install -y gfortran
COPY --from=builder /usr/local/cargo/bin/thumbnail-service-rs /usr/local/bin/thumbnail-service-rs
CMD [ "thumbnail-service-rs" ]
