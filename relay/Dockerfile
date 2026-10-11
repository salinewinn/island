FROM rust:1-slim AS build
WORKDIR /src
COPY Cargo.toml .
COPY src ./src
RUN cargo build --release

FROM debian:stable-slim
COPY --from=build /src/target/release/island-relay /usr/local/bin/island-relay
EXPOSE 47400
CMD ["island-relay", "47400"]
