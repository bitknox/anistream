# syntax=docker/dockerfile:1

# The plugin store server, `anistream-store`, as a container image: what a VPS or NAS, Fly.io,
# Railway, Render and Heroku run. Build it from the repository root:
#
#   docker build -f deploy/container/Dockerfile -t anistream-store .
#
# It builds for the platform the builder runs on. CI builds linux/amd64 and linux/arm64, each on
# a runner of that architecture, and publishes both as ghcr.io/bitknox/anistream-store
# (.github/workflows/store-image.yml).

# This tag only picks the image the build starts from. rust-toolchain.toml decides the compiler:
# the `rustup toolchain install` below installs whatever it pins, so a bump there costs one
# download until this line catches up, never a build with the wrong toolchain.
ARG RUST_VERSION=1.99.1

FROM rust:${RUST_VERSION}-slim-trixie AS build
WORKDIR /src

# In its own layer, so a source change does not download the toolchain again.
COPY rust-toolchain.toml ./
RUN rustup toolchain install

# The slim image's gcc and libc headers are all the C parts of the build need: bundled SQLite,
# zstd, ring, and aws-lc, which needs neither CMake nor clang on Linux.
COPY . .
ARG TARGETARCH
# The cache mounts keep downloaded crates and compiled dependencies between builds on one
# machine. The target directory is kept per architecture, so a builder making both images never
# mixes their artefacts. Mounts are not part of the image, so the binary is copied out in the
# same step. The empty data directory is made here because the runtime image has no shell.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/src/target,id=anistream-store-target-${TARGETARCH},sharing=locked \
    cargo build --release --locked -p anistream-store-server \
    && install -D -m 0755 target/release/anistream-store /out/bin/anistream-store \
    && mkdir -p /out/volume/data

# glibc, libgcc and CA certificates, which is everything the binary links against, and no shell or
# package manager. Debian 13 like the build stage, so the binary runs on the glibc it was linked
# against. /tmp is world-writable here, which the plugin inspector needs for its scratch files.
FROM gcr.io/distroless/cc-debian13:nonroot

# GHCR links the package to the repository through `source`.
LABEL org.opencontainers.image.title="anistream-store" \
      org.opencontainers.image.description="A private plugin store for anistream. Friends join with an invite link and install the plugins its owner publishes." \
      org.opencontainers.image.source="https://github.com/bitknox/anistream" \
      org.opencontainers.image.url="https://github.com/bitknox/anistream" \
      org.opencontainers.image.documentation="https://github.com/bitknox/anistream/tree/main/deploy" \
      org.opencontainers.image.licenses="MIT"

COPY --from=build /out/bin/anistream-store /usr/local/bin/anistream-store
COPY LICENSE /usr/share/doc/anistream-store/LICENSE
# /data as a directory owned by the user the store runs as. Docker copies a mount point's
# ownership into a new named volume, so the store can write there without root. Copied as the
# contents of /out/volume so that --chown applies to the directory itself.
COPY --from=build --chown=65532:65532 /out/volume/ /

# The default directory comes last in the store's search: DATABASE_URL wins over it, so the same
# image keeps everything in Postgres on Heroku, which has no disk. PORT makes the store listen on
# every interface, and platforms that assign their own port replace it.
ENV ANISTREAM_STORE_DEFAULT_DIR=/data \
    PORT=7878

# distroless's `nonroot`, by number, so that a runtime can check it is not root without reading
# /etc/passwd.
USER 65532:65532
VOLUME /data
EXPOSE 7878

ENTRYPOINT ["anistream-store"]
CMD ["serve"]
