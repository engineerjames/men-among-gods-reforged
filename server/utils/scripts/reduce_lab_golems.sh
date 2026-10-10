# Lab 7 (Water Gorge) and Lab 8 (Forest Gorge) golems: bonuses 70 -> 55, rank Major -> Captain (11).
# Areas from core/src/area.rs; Lab 8 is split in two rectangles.
for area in 16,529,81,591 15,611,126,703 112,703,126,708; do
  cargo run -q -p server-utils --bin template-search -- --chars --name golem --area "$area" \
    --set-armor-bonus 55 --set-weapon-bonus 55 --set-rank 11 --write "$@"
done
