# Golem character templates that spawn in Lab 7 (Water Gorge) and Lab 8 (Forest Gorge); areas from core/src/area.rs.
echo "Lab 7"
cargo run -q -p server-utils --bin template-search -- --chars --name golem --area 16,529,81,591 "$@"
echo "Lab 8"
cargo run -q -p server-utils --bin template-search -- --chars --name golem --area 15,611,126,703 "$@"
cargo run -q -p server-utils --bin template-search -- --chars --name golem --area 112,703,126,708 "$@" | tail -n +2
