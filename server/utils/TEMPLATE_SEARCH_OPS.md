# template-search `--ops` files

`--ops FILE.yaml` runs a list of `template-search` operations against one loaded
snapshot and saves once at the end, instead of re-parsing the `.wsnap` per call.

```sh
cargo run -q -p server-utils --bin template-search -- --ops ops.yaml          # dry run
cargo run -q -p server-utils --bin template-search -- --ops ops.yaml --write  # save
```

## Syntax

The file is a YAML list. Each entry is a map of the normal CLI flags, without the leading `--`:

| YAML value            | Becomes                      |
| --------------------- | ---------------------------- |
| `key: 5` / `key: abc` | `--key 5` / `--key abc`      |
| `key: true`           | `--key` (switch)             |
| `key: false`          | omitted                      |
| `key: [a, b]`         | `--key a --key b` (repeated) |

- Operations run in order on the same in-memory snapshot, so later ones see earlier edits.
- All operations are parsed first; a bad flag aborts the whole batch (exit 2) before anything runs.
- `snapshot`, `write`, `output` and `ops` are batch-level: pass them on the command line, not in an entry.
- Values containing commas that YAML might misread (e.g. `area`) are safest quoted: `area: "15,611,126,703"`.
- Edit rules are unchanged: `--chars` edits need `ids` or `name`; `set-wv` edits items and needs `ids` without `chars`.

## Examples

Set weapon values on item templates:

```yaml
- {ids: 501, set-wv: 2}
- {ids: 158, set-wv: 4}
```

Zero the bonuses on ranges of character templates:

```yaml
- chars: true
  ids: 364-374,1094-1096
  set-armor-bonus: 0
  set-weapon-bonus: 0
```

Repeated flags and a YAML anchor to reuse one operation with a different area:

```yaml
- &golems
  chars: true
  name: golem
  area: 16,529,81,591
  set-rank: 11
  cap-attrib: [STREN=65, AGIL=65]
  cap-skill: [hand=90, weapon=90]
- {<<: *golems, area: "15,611,126,703"}
```

Pure searches work too (output is printed per operation, no save):

```yaml
- {name: potion, flag: IF_MAGIC}
```

See [scripts/reduce_pent_difficulty.yaml](scripts/reduce_pent_difficulty.yaml) and
[scripts/reduce_lab_golems.yaml](scripts/reduce_lab_golems.yaml) for real batches.
