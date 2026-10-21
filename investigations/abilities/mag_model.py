"""Shared ports of the server's combat / spell primitives for the ability notebooks.

Every helper is vectorised over numpy arrays and reproduces the Rust integer
semantics (truncating division, explicit clamps, ``as i32`` truncation) so the
notebooks can assert exact values taken from the server unit tests.

Rust sources are referenced relative to the repository root.
"""

from __future__ import annotations

import re
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

#: Server ticks per second (`core::constants::TICKS`).
TICKS = 36

#: Modified skill values (`skill[n][5]`) plotted on every x-axis. Top-rank
#: players reach about 275-300 modified, and gear can push past that.
SKILL_RANGE = np.arange(1, 351)

#: Modified skill at which ability curves reach their design value
#: (`driver::skill::common::SKILL_KNEE`).
SKILL_KNEE = 300

#: Typical endgame modified skill band, shaded on every skill axis.
ENDGAME = (275, 300)

#: Raise caps: 105 for most classes, 120 for Mercenary skills.
RAISE_CAP = 105
RAISE_CAP_MERC = 120
CAPS = (RAISE_CAP, RAISE_CAP_MERC)

#: A melee swing occupies 8 action advances (attack status 160..167).
ATTACK_ADVANCES = 8

REPO_ROOT = Path(__file__).resolve().parents[2]


def _scalar(x):
    """Unwrap 0-d arrays so scalar inputs give scalar outputs."""
    arr = np.asarray(x)
    if arr.ndim == 0:
        return arr.item()
    return arr


# ---------------------------------------------------------------------------
# Integer / random helpers
# ---------------------------------------------------------------------------


def knee_300(p, at_0, at_300):
    """`skill_knee_300`: linear from ``at_0`` to ``at_300`` over modified skill
    0..300, then half that slope. Truncates toward zero like the Rust i64 math."""
    p = np.maximum(np.asarray(p, dtype=np.int64), 0)
    span = int(at_300) - int(at_0)
    below = at_0 + idiv(span * p, SKILL_KNEE)
    above = at_300 + idiv(span * (p - SKILL_KNEE), 2 * SKILL_KNEE)
    return _scalar(np.where(p <= SKILL_KNEE, below, above))


def idiv(a, b):
    """Rust `i32 / i32`: integer division truncating toward zero.

    Python's ``//`` floors, which differs for negative operands (e.g. -7 / 2).
    """
    a = np.asarray(a, dtype=np.int64)
    b = np.asarray(b, dtype=np.int64)
    q = np.abs(a) // np.abs(b)
    return _scalar(np.where((a < 0) ^ (b < 0), -q, q))


def clamp(x, lo, hi):
    """`i32::clamp`."""
    return _scalar(np.clip(np.asarray(x), lo, hi))


def rand_mod_mean(n):
    """Mean of `helpers::random_mod_i32(n)` (uniform over `0..n`, 0 when n <= 0)."""
    n = np.asarray(n, dtype=float)
    return _scalar(np.where(n > 0, (n - 1) / 2, 0.0))


def rand_mod_range(n):
    """Inclusive (min, max) of `helpers::random_mod_i32(n)`."""
    n = np.asarray(n)
    return _scalar(np.zeros_like(n)), _scalar(np.maximum(n - 1, 0))


# ---------------------------------------------------------------------------
# Spell power (driver/skill/common.rs)
# ---------------------------------------------------------------------------

#: `spell_race_mod` multipliers, checked in this priority order in Rust.
RACE_MOD = {
    "ArchHarakim": 1.05,
    "ArchTemplar": 0.95,
    "Sorcerer": 1.10,
    "Warrior": 1.10,
    "Seyan'Du": 0.95,
    "Harakim": 1.00,
    "Mercenary": 1.05,
    "Templar": 0.90,
    "Other": 1.00,
}


def spell_immunity(power, immun):
    """`spell_immunity`: `power - immun/2`, or 1 when immunity swallows it."""
    power = np.asarray(power)
    half = idiv(immun, 2)
    return _scalar(np.where(power <= half, 1, power - half))


def spell_race_mod(power, race="Other", moon=None):
    """`spell_race_mod`: `(power as f64 * modf) as i32`.

    ``moon`` is ``None``, ``"new"`` (-0.15) or ``"full"`` (+0.15).
    """
    modf = RACE_MOD[race]
    if moon == "new":
        modf -= 0.15
    if moon == "full":
        modf += 0.15
    return _scalar(np.trunc(np.asarray(power, dtype=float) * modf).astype(np.int64))


def spell_power(skill, immun=0, race="Other", moon=None):
    """p' = race(immun(skill)) — the power most offensive spells store."""
    return spell_race_mod(spell_immunity(skill, immun), race, moon)


def effective_resist(raw, pen_pct=0):
    """`effective_resist`: resist scaled down by the caster's spell penetration %."""
    if np.all(np.asarray(pen_pct) == 0):
        return _scalar(np.asarray(raw))
    return idiv(np.asarray(raw) * 100, 100 + np.asarray(pen_pct))


def p_chance_base(skill, d20, power, luck=0):
    """Exact success probability of `chance_base(cn, skill, d20, power)`.

    `chance = clamp(d20*skill/max(1,power) [+ luck/500 - 1], 0, 18)`; the roll
    `random_mod(20)` fails when it exceeds `chance`, and the cast always fails when
    `power > skill + skill/2`.
    """
    skill = np.asarray(skill, dtype=np.int64)
    power = np.asarray(power, dtype=np.int64)
    chance = idiv(d20 * skill, np.maximum(1, power))
    if luck < 0:
        chance = chance + idiv(luck, 500) - 1
    chance = np.clip(chance, 0, 18)
    p = (chance + 1) / 20
    return _scalar(np.where(power > skill + idiv(skill, 2), 0.0, p))


def spell_cost(cost, concen=0):
    """Mana actually spent by `spellcost(cost)` for a caster with Concentration ``concen``.

    Concentration removes `cost*concen/300`; ``concen=0`` means the skill is unlearned.
    """
    cost = np.asarray(cost, dtype=np.int64)
    t = idiv(cost * np.asarray(concen), 300)
    return _scalar(np.where(t > cost, 1, cost - t))


def p_opposed_d20(power, resist):
    """P(`power + rand(20) > resist + rand(20)`) — the adjacent-attacker check."""
    power = np.asarray(power)[..., None, None]
    resist = np.asarray(resist)[..., None, None]
    a = np.arange(20)[:, None]
    b = np.arange(20)[None, :]
    return _scalar(((power + a) > (resist + b)).mean(axis=(-2, -1)))


# ---------------------------------------------------------------------------
# Damage (state/stats.rs::do_hurt)
# ---------------------------------------------------------------------------

#: milli-HP per point of post-armor damage, by `do_hurt` type.
HURT_SCALE = {0: 250, 1: 750, 2: 750, 3: 1000}


def hurt_hp(dam, armor=0, kind=0):
    """HP removed by `do_hurt(dam, kind)` against ``armor``.

    type 0 (melee): `(dam - armor) * 250` milli-HP; type 1/2 (spells): `* 750`;
    type 3 (true damage): `dam * 1000`, armor ignored. Clamped at 0.
    """
    dam = np.asarray(dam, dtype=float)
    if kind == 3:
        return _scalar(np.maximum(dam, 0.0))
    return _scalar(np.maximum(dam - np.asarray(armor), 0.0) * HURT_SCALE[kind] / 1000)


def expected_hurt_uniform(lo, n, armor=0, kind=0):
    """E[`hurt_hp(lo + random_mod(n), armor, kind)`], exact.

    The armor clamp makes the expectation non-linear, so this sums the clamped
    uniform distribution in closed form rather than plugging in the mean damage.
    """
    lo, n, armor = np.broadcast_arrays(
        np.asarray(lo, dtype=np.int64),
        np.asarray(n, dtype=np.int64),
        np.asarray(armor, dtype=np.int64),
    )
    if kind == 3:
        return _scalar(lo + rand_mod_mean(n))
    n_eff = np.maximum(n, 1)  # random_mod(<=0) always returns 0
    a = armor - lo  # roll k contributes max(0, k - a)
    k0 = np.maximum(0, a)
    m = np.maximum(n_eff - k0, 0)
    total = m * (k0 - a) + m * (m - 1) / 2
    return _scalar(total / n_eff * HURT_SCALE[kind] / 1000)


def dot_tick(power, kind="parasite"):
    """HP per second of a Parasite-family DoT item (`stats.rs`, ignores armor).

    `base = max(1, 3*power/4)`; Contagion deals `2*base`, Lava burn `max(1, base/2)`.
    Rains of Renewal heals with the same `base` formula.
    """
    base = np.maximum(1, idiv(np.asarray(power) * 3, 4))
    if kind == "contagion":
        return _scalar(base * 2)
    if kind == "lava":
        return _scalar(np.maximum(1, idiv(base, 2)))
    return _scalar(base)


# ---------------------------------------------------------------------------
# Melee (state/combat.rs)
# ---------------------------------------------------------------------------

# (upper bound exclusive, chance, bonus); Δ == 0 handled separately.
_HIT_TABLE_NEG = [
    (-120, 1, -64),
    (-80, 1, -32),
    (-40, 1, -16),
    (-36, 2, -8),
    (-32, 3, -4),
    (-28, 4, -2),
    (-24, 5, -1),
    (-20, 6, 0),
    (-16, 7, 0),
    (-12, 8, 0),
    (-8, 9, 0),
    (-4, 10, 0),
    (0, 11, 0),
]
_HIT_TABLE_POS = [
    (4, 13, 0),
    (8, 14, 0),
    (12, 15, 0),
    (16, 16, 1),
    (20, 17, 2),
    (24, 18, 3),
    (28, 19, 4),
    (32, 19, 5),
    (36, 19, 10),
    (40, 19, 15),
]


def _hit_row(d):
    if d == 0:
        return 12, 0
    for bound, chance, bonus in _HIT_TABLE_NEG if d < 0 else _HIT_TABLE_POS:
        if d < bound:
            return chance, bonus
    return 19, 20


def melee_hit_table(delta):
    """Δskill (attacker − defender fight skill) → (chance on d20, damage bonus)."""
    d = np.asarray(delta, dtype=np.int64)
    rows = np.vectorize(_hit_row, otypes=[np.int64, np.int64])(d)
    return _scalar(rows[0]), _scalar(rows[1])


def p_melee_hit(delta):
    """P(`die <= chance`) with `die = random_mod(20) + 1`."""
    chance, _ = melee_hit_table(delta)
    return _scalar(np.asarray(chance) / 20)


def melee_raw_rolls(wv, strength):
    """Equally likely raw damages of a normal melee hit (die >= 3), before the table bonus.

    `WV + rand(6)+1 + rand(STR/2)` (the strength term only when STR > 3). This is
    `odam` in `do_attack`, the value Surround Hit derives from.
    """
    a = np.arange(6) + 1
    b = np.arange(max(1, strength // 2)) if strength > 3 else np.array([0])
    return (wv + a[:, None] + b[None, :]).ravel()


def melee_hit_hp(wv, strength, armor=0, bonus=0):
    """Expected HP removed by one normal landed melee hit (die >= 3) against ``armor``.

    ``armor`` may be an array; ``wv`` and ``strength`` are scalars.
    """
    rolls = melee_raw_rolls(wv, strength) + bonus
    armor = np.asarray(armor)
    return _scalar(np.asarray(hurt_hp(rolls[None, :], armor.reshape(-1, 1))).mean(axis=-1).reshape(armor.shape))


def p_rand20_exceeds(skill, target):
    """P(`skill + random_mod(20) > target`), e.g. Surround Hit vs the victim's fight skill."""
    margin = np.asarray(target) - np.asarray(skill)
    return _scalar(np.clip(19 - margin, 0, 20) / 20)


# ---------------------------------------------------------------------------
# Speed (core/src/constants.rs SPEEDTAB, state/stats.rs::really_update_char)
# ---------------------------------------------------------------------------


def _load_speedtab():
    src = (REPO_ROOT / "core/src/constants.rs").read_text()
    body = re.search(r"pub const SPEEDTAB[^=]*=\s*\[(.*?)\];", src, re.S).group(1)
    rows = re.findall(r"\[([0-9,\s]+)\]", body)
    table = np.array([[int(v) for v in r.split(",") if v.strip()] for r in rows])
    assert table.shape == (20, 40), table.shape
    return table


#: 20 rows × 40 columns of 0/1 "may advance this tick" flags, parsed from Rust.
SPEEDTAB = _load_speedtab()


def speed_table():
    """Return the parsed `SPEEDTAB` (rows = speed index, 0 fastest)."""
    return SPEEDTAB


def _round_half_away(x):
    """`f32::round` (half away from zero), unlike Python's banker's rounding."""
    x = np.asarray(x, dtype=float)
    return np.sign(x) * np.floor(np.abs(x) + 0.5)


MODE_OFFSET = {"sneak": 12, "normal": 14, "fast": 16}


def base_speed(agil, stren, speed_mod=0, mode="normal"):
    """`base_speed_calc = (agil + stren)/50 + speed_mod + {12,14,16}`."""
    return _scalar(idiv(np.asarray(agil) + np.asarray(stren), 50) + speed_mod + MODE_OFFSET[mode])


def action_speed_row(base, attack_pct=0, flash_haste=False, flash_slow=0):
    """`future3[2]`: the SPEEDTAB row that gates attacks and other actions."""
    base = np.asarray(base)
    calc = base + _round_half_away(base * attack_pct / 100).astype(np.int64)
    calc = calc + (3 if flash_haste else 0) - np.asarray(flash_slow)
    return _scalar(np.clip(20 - calc, 0, 19))


def movement_speed_row(base, move_pct=0, flash_haste=False):
    """`speed`: the SPEEDTAB row that gates walking."""
    base = np.asarray(base)
    calc = base + _round_half_away(base * move_pct / 100).astype(np.int64) + int(flash_haste)
    return _scalar(np.clip(20 - calc, 0, 19))


def advances_per_second(row):
    """Action advances per second for a SPEEDTAB row (`36 * sum(row) / 40`)."""
    return _scalar(TICKS * SPEEDTAB.sum(axis=1)[np.asarray(row)] / 40)


def attacks_per_second(row):
    """Melee swings per second for an action-speed row (8 advances per swing)."""
    return _scalar(np.asarray(advances_per_second(row)) / ATTACK_ADVANCES)


# ---------------------------------------------------------------------------
# Plotting
# ---------------------------------------------------------------------------

#: Default target armor / immunity sweeps used across notebooks.
ARMOR_SWEEP = (0, 20, 40, 60, 80)
IMMUNITY_SWEEP = (0, 25, 50, 75, 100)
WV_SWEEP = (10, 20, 40, 60, 80)
RESIST_SWEEP = (25, 50, 75, 100, 150)


def apply_style():
    """Shared matplotlib style for the ability notebooks."""
    plt.rcParams.update(
        {
            "figure.figsize": (9, 5),
            "figure.dpi": 100,
            "axes.grid": True,
            "grid.alpha": 0.3,
            "axes.spines.top": False,
            "axes.spines.right": False,
            "axes.titleweight": "bold",
            "legend.frameon": False,
            "lines.linewidth": 2,
        }
    )


def mark_caps(ax, extra=(), caps=CAPS, label=True):
    """Dotted verticals at the raise caps, the shaded endgame band, plus any
    formula saturation points.

    ``extra`` is an iterable of ``x`` or ``(x, text)`` entries.
    """
    ax.axvspan(*ENDGAME, color="tab:green", alpha=0.08, lw=0)
    if label:
        ax.text(ENDGAME[0], 0.02, " endgame", transform=ax.get_xaxis_transform(),
                va="bottom", ha="left", fontsize=8, color="tab:green")
    for i, c in enumerate(caps):
        ax.axvline(c, color="grey", ls=":", lw=1)
        if label:
            ax.text(c, 1.0 - 0.05 * i, f" cap {c}", transform=ax.get_xaxis_transform(),
                    va="top", ha="left", fontsize=8, color="grey")
    for i, e in enumerate(extra):
        x, text = e if isinstance(e, tuple) else (e, None)
        ax.axvline(x, color="tab:red", ls=":", lw=1)
        if text and label:
            ax.text(x, 0.88 - 0.05 * i, f" {text}", transform=ax.get_xaxis_transform(),
                    va="top", ha="left", fontsize=8, color="tab:red")


def skill_axis(ax, ylabel, title=None):
    """Label an axis whose x is the caster's modified skill value."""
    ax.set_xlabel("caster modified skill (skill[n][5])")
    ax.set_ylabel(ylabel)
    ax.set_xlim(SKILL_RANGE[0], SKILL_RANGE[-1])
    if title:
        ax.set_title(title)
