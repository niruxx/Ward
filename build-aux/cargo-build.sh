#!/bin/sh
set -eu
source_dir="$1"
output_dir="$2"
cargo build --manifest-path "$source_dir/Cargo.toml" --target-dir "$output_dir/cargo" --release --locked
cp "$output_dir/cargo/release/ward" "$output_dir/ward"
cp "$output_dir/cargo/release/ward-helper" "$output_dir/ward-helper"
