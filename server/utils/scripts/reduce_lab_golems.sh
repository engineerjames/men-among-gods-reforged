# Lab 7 (Water Gorge) and Lab 8 (Forest Gorge) golems: bonuses 70 -> 55, rank Major -> Captain (11),
# strength/agility 75 -> 65, hand-to-hand/weapon skill 100 -> 90 (caps only ever lower a value).
# Areas from core/src/area.rs; Lab 8 is split in two rectangles.
for area in 16,529,81,591 15,611,126,703 112,703,126,708; do
  cargo run -q -p server-utils --bin template-search -- --chars --name golem --area "$area" \
    --set-armor-bonus 55 --set-weapon-bonus 55 --set-rank 11 \
    --cap-attrib STREN=65 --cap-attrib AGIL=65 --cap-skill hand=90 --cap-skill weapon=90 --write "$@"
done
