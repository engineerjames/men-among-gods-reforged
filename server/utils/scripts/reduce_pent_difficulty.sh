# Weapon values (WV) for the weapons grolms and sea golems carry; edit and re-run to retune.
BRONZE_AXE_WV=2   # id 501, was 10
STEEL_AXE_WV=4    # id 158, was 20
GOLD_AXE_WV=8     # id 502, was 30 (id 1207 Tower Golden Axe is deliberately left alone)
LAVA_SWORD_WV=10  # id 442, was 42
ICE_SWORD_WV=40   # id 813, was 80

TS="cargo run -q -p server-utils --bin template-search --"
$TS --ids 501 --set-wv "$BRONZE_AXE_WV" --write
$TS --ids 158 --set-wv "$STEEL_AXE_WV" --write
$TS --ids 502 --set-wv "$GOLD_AXE_WV" --write
$TS --ids 442 --set-wv "$LAVA_SWORD_WV" --write
$TS --ids 813 --set-wv "$ICE_SWORD_WV" --write

# Non-gargoyles (grolms 364-374, sea golems 1094-1096): keep weapon, zero bonuses
$TS --chars --ids 364-374,1094-1096 --set-armor-bonus 0 --set-weapon-bonus 0 --write

# Gargoyles (375-381) and ice gargoyles (539-542): keep weapon, zero bonuses
$TS --chars --ids 375-381,539-542 --set-armor-bonus 0 --set-weapon-bonus 0 --write