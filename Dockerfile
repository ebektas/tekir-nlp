# HTTP JSON server image. Lexicon and disambiguation model are embedded in
# the binary, so the runtime image holds nothing else.
#   docker build -t tekir-nlp .
#   docker run --rm -p 8321:8321 tekir-nlp

FROM rust:1.95-slim-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p tekir-cli --features server

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /src/target/release/tekir-cli /usr/local/bin/tekir
EXPOSE 8321
ENTRYPOINT ["tekir", "serve", "--host", "0.0.0.0", "--port", "8321"]
