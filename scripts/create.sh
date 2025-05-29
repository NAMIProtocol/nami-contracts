#!/bin/bash

# Set the replacement string from the first argument
replacement=$1

# Replace hyphens with underscores in the replacement string for Rust compatibility
rust_friendly_replacement=${replacement//-/_}

# Use $rust_friendly_replacement where Rust module or filename is needed
cp -r contracts/nami-template contracts/nami-$replacement
cp packages/nami-rs/src/interfaces/template.rs packages/nami-rs/src/interfaces/$rust_friendly_replacement.rs
cd contracts/nami-$replacement
grep -rl 'nami-template' . | xargs sed -i '' "s/nami-template/nami-$replacement/g"
grep -rl 'nami_rs::template' . | xargs sed -i '' "s/nami_rs::template/nami_rs::$rust_friendly_replacement/g"
grep -rl 'template::' ./src/bin/schema.rs | xargs sed -i '' "s/template::/$rust_friendly_replacement::/g"
awk -v new_line="pub mod ${rust_friendly_replacement};" 'NR==1 {print new_line} {print}' ../../packages/nami-rs/src/interfaces/mod.rs > temp && mv temp ../../packages/nami-rs/src/interfaces/mod.rs
cargo run schema
cd -
cargo fmt
