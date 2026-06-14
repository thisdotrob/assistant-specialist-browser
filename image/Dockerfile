# assistant-specialist-browser: the browser specialist's custom image. It is the slim
# `assistant-base` plus only the binaries this specialist needs — a headless
# Chromium and the `agent-browser` CLI that drives it. Node, the Claude Code CLI,
# the shim source, and the entrypoint are all inherited from the base, so this
# image adds binaries and nothing else. Built outside the sandbox (apt + npm need
# network); the base tag must match the `assistant-base` build.

ARG BASE_IMAGE=assistant-base:0.1.0
FROM ${BASE_IMAGE}

# Pinned so an upstream release can't silently change behaviour under a rebuild.
ARG AGENT_BROWSER_PACKAGE=agent-browser@0.27.2

# Chromium + the shared libraries headless Chromium needs at runtime. This heavy,
# network-bound layer is the whole reason the browser specialist ships its own
# image instead of running on the slim base.
RUN apt-get update && apt-get install -y \
    chromium \
    fonts-liberation \
    fonts-noto-color-emoji \
    libgbm1 \
    libnss3 \
    libatk-bridge2.0-0 \
    libgtk-3-0 \
    libx11-xcb1 \
    libxcomposite1 \
    libxdamage1 \
    libxrandr2 \
    libasound2 \
    libpangocairo-1.0-0 \
    libcups2 \
    libdrm2 \
    libxshmfence1 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN npm install -g "${AGENT_BROWSER_PACKAGE}"

# Point agent-browser (and the Playwright layer it uses) at the distro Chromium
# so neither downloads a separate browser build.
ENV AGENT_BROWSER_EXECUTABLE_PATH=/usr/bin/chromium
ENV PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH=/usr/bin/chromium

# CLAW_SESSION_DIR and the ENTRYPOINT are inherited from assistant-base.
