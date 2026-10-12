# Operations live in reduce_pent_difficulty.yaml; edit and re-run to retune.
cargo run -q -p server-utils --bin template-search -- --ops "$(dirname "$0")/reduce_pent_difficulty.yaml" --write
