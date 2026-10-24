# A Linux box to build and run the desktop app in: GTK 4, libadwaita and libmpv from Debian, a
# virtual X display and Mesa's software GL, so the app runs headless with no GPU.
#
#   docker build -t anistream-linux:dev desktop/linux/docker
FROM debian:trixie

RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential pkg-config curl ca-certificates git cmake clang libclang-dev nasm perl \
      libgtk-4-dev libadwaita-1-dev libmpv-dev libegl-dev \
      mpv ffmpeg \
      xvfb xauth x11-apps imagemagick weston \
      libgl1-mesa-dri libegl-mesa0 mesa-utils \
      adwaita-icon-theme librsvg2-common fonts-cantarell fonts-inter dbus \
    && rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo PATH=/opt/cargo/bin:$PATH
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.99.1 \
      --component clippy,rustfmt && chmod -R a+rwX /opt/cargo /opt/rustup
