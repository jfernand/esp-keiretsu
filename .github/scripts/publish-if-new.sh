#!/usr/bin/env bash
# Publishes a workspace crate to crates.io, skipping quietly if the version currently in its
# Cargo.toml is already published there -- so one release tag can publish only whichever bsp
# crate(s) actually had their version bumped, instead of failing the whole job on the other.
set -euo pipefail

crate="$1"

version=$(cargo metadata --no-deps --format-version 1 |
    jq -r --arg name "$crate" '.packages[] | select(.name == $name) | .version')

if [ -z "$version" ]; then
    echo "::error::no package named '$crate' found in workspace metadata"
    exit 1
fi

echo "== $crate $version =="

status=$(curl -s -o /dev/null -w '%{http_code}' \
    -A "esp-keiretsu-release-script (github.com/jfernand/esp-keiretsu)" \
    "https://crates.io/api/v1/crates/$crate/$version")

if [ "$status" = "200" ]; then
    echo "$crate $version is already published on crates.io -- skipping."
    exit 0
fi

echo "Publishing $crate $version..."
# Run from firmware/ so cargo picks up its .cargo/config.toml (riscv32imac target + build-std
# core/alloc) -- these crates have no target config of their own since Cargo only discovers
# .cargo/config.toml from the current working directory upward, not per workspace member.
(cd firmware && cargo publish -p "$crate")
