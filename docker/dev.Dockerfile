# Formavium Entwicklungsumgebung (Linux-Container).
# Enthält Rust, Node.js (für das spätere Svelte-Frontend) und die Tectonic-CLI
# in fester Version.
FROM rust:1.91-bookworm

ARG TECTONIC_VERSION=0.17.0
ARG NODE_MAJOR=22

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      ca-certificates curl xz-utils poppler-utils fontconfig \
 && curl -fsSL https://deb.nodesource.com/setup_${NODE_MAJOR}.x | bash - \
 && apt-get install -y --no-install-recommends nodejs \
 && rm -rf /var/lib/apt/lists/*

RUN curl -fsSL "https://github.com/tectonic-typesetting/tectonic/releases/download/tectonic%40${TECTONIC_VERSION}/tectonic-${TECTONIC_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
      | tar -xz -C /usr/local/bin tectonic \
 && tectonic --version

# Systembibliotheken für Tauri 2 unter Linux (Prüfbau der Desktop-Hülle).
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev \
      libgtk-3-dev librsvg2-dev libxdo-dev libssl-dev file \
 && rm -rf /var/lib/apt/lists/*

# Windows-Cross-Build: MinGW-w64-Linker und Ressourcencompiler,
# unzip für die Tectonic-Windows-Release.
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      gcc-mingw-w64-x86-64 g++-mingw-w64-x86-64 binutils-mingw-w64-x86-64 unzip \
      scons zlib1g-dev \
 && rm -rf /var/lib/apt/lists/*

# NSIS: Debian liefert 3.08, Tauri braucht Includes ab 3.09. makensis wird
# für Linux aus den Quellen gebaut und mit Stubs, Plugins und Includes der
# offiziellen Windows-Distribution derselben Version kombiniert.
ARG NSIS_VERSION=3.10
RUN set -eux; cd /tmp; \
    base="https://downloads.sourceforge.net/project/nsis/NSIS%203/${NSIS_VERSION}"; \
    curl -fsSL --retry 5 --retry-all-errors -o src.tar.bz2 "$base/nsis-${NSIS_VERSION}-src.tar.bz2"; \
    curl -fsSL --retry 5 --retry-all-errors -o data.zip "$base/nsis-${NSIS_VERSION}.zip"; \
    unzip -q data.zip; mv "nsis-${NSIS_VERSION}" /opt/nsis; \
    tar -xjf src.tar.bz2; cd "nsis-${NSIS_VERSION}-src"; \
    scons -Q VERSION=${NSIS_VERSION} SKIPSTUBS=all SKIPPLUGINS=all SKIPUTILS=all SKIPMISC=all SKIPDOC=all \
          NSIS_CONFIG_CONST_DATA_PATH=no PREFIX=/opt/nsis makensis >/dev/null; \
    cp build/urelease/makensis/makensis /opt/nsis/makensis; \
    printf '#!/bin/sh\nexport NSISDIR=/opt/nsis\nexec /opt/nsis/makensis "$@"\n' > /usr/local/bin/makensis; \
    chmod +x /usr/local/bin/makensis; \
    cd /tmp; rm -rf src.tar.bz2 data.zip "nsis-${NSIS_VERSION}-src"; \
    makensis -VERSION

RUN rustup component add rustfmt clippy \
 && rustup target add x86_64-pc-windows-gnu

ENV CARGO_TARGET_DIR=/target \
    TECTONIC_CACHE_DIR=/tectonic-cache
WORKDIR /work
