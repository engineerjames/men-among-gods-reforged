# Operations live in reduce_lab_golems.yaml; edit and re-run to retune. Extra args (e.g. --output) pass through.
cargo run -q -p server-utils --bin template-search -- --ops "$(dirname "$0")/reduce_lab_golems.yaml" --write "$@"
