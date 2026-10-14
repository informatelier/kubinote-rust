# O3-KR3: immagine del server KubiRAG sotto i 100 MB.
#
# Il contesto deve contenere kubi-kernel, che è privato: la CI lo mette in `.deps/` con il
# patch in `.cargo/config.toml` (vedi .github/workflows/ci.yml); in locale si ottiene lo
# stesso con un checkout di kubicommons-rust in `.deps/kubicommons-rust`.
FROM rust:1-slim-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release -p kubirag-cli && cp target/release/kubirag /kubirag

# glibc e libgcc, niente shell: il binario non ha altre dipendenze dinamiche.
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /kubirag /usr/local/bin/kubirag
# Come l'immagine Kotlin: configurazione locale in /data/qb, Ollama sull'host.
ENV QB_ROOT_PATH=/data/qb/ \
    OLLAMA_BASE_URL=http://host.docker.internal:11434/api
WORKDIR /data
VOLUME ["/data"]
EXPOSE 7070
ENTRYPOINT ["/usr/local/bin/kubirag", "server"]
