#!/bin/sh
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Benjamin Zores
# Set the package version in Cargo.toml and Cargo.lock.
# Used by semantic-release (prepareCmd) before the release commit.
set -eu
v="${1:?usage: set-version.sh <version>}"
root="$(cd "$(dirname "$0")/.." && pwd)"

# [package] version (first `version =` line of Cargo.toml)
awk -v v="$v" '!done && /^version = "/ { print "version = \"" v "\""; done = 1; next } { print }' \
  "$root/Cargo.toml" > "$root/Cargo.toml.tmp" && mv "$root/Cargo.toml.tmp" "$root/Cargo.toml"

# the tmacs entry of Cargo.lock
awk -v v="$v" '
  /^name = "tmacs"$/ { print; getline; print "version = \"" v "\""; next } { print }
' "$root/Cargo.lock" > "$root/Cargo.lock.tmp" && mv "$root/Cargo.lock.tmp" "$root/Cargo.lock"

grep -q "^version = \"$v\"" "$root/Cargo.toml"
echo "tmacs version set to $v"
