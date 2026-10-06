# Non-gargoyles (grolms 364-374, sea golems 1094-1096): remove weapon, zero bonuses
cargo run -p server-utils --bin template-search -- --chars --ids 364-374,1094-1096 \
  --clear-worn RHAND --set-armor-bonus 0 --set-weapon-bonus 0 --write

# Gargoyles (375-381) and ice gargoyles (539-542): keep weapon, zero bonuses
cargo run -p server-utils --bin template-search -- --chars --ids 375-381,539-542 \
  --set-armor-bonus 0 --set-weapon-bonus 0 --write