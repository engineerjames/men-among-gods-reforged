#!/usr/bin/env python3
"""Analyze a `perf_loadtest.sh` run directory and report server bottlenecks.

Reads the artifacts produced by ``scripts/perf_loadtest.sh``:

* ``server-logs/server_perf.log`` (plus rotated ``.1``..``.5`` siblings) —
  ``measure!`` timings, tick-time samples and network-I/O samples.
* ``run_meta.json`` — the steady-state window to restrict analysis to.
* ``server_resources.csv`` — CPU / RSS samples of the server process.
* ``sample.txt`` — optional macOS ``sample`` call-tree capture.
* ``loadtest.log`` — optional client-side metrics summary.

Only the Python standard library is used.

Usage:
    python3 scripts/analyze_perf_log.py --run-dir perf-runs/2026-09-28_10-00-00
"""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
from datetime import datetime, timezone
from pathlib import Path

# ---------------------------------------------------------------------------
# Log line grammar
# ---------------------------------------------------------------------------

#  2026-09-28T10:00:00.123456789 INFO server/src/server.rs:595 - <message>
LINE_RE = re.compile(
    r"^(?P<ts>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?)\s+"
    r"(?P<level>\w+)\s+(?P<loc>\S+)\s+-\s+(?P<msg>.*)$"
)

MEASURE_RE = re.compile(r"^\[measure-time\]\s+(?P<label>.+?)\s+took\s+(?P<dur>\S+)$")

TICK_RE = re.compile(
    r"^Tick time:\s+(?P<tick>[\d.]+)\s+ms\s+\(max:\s+(?P<max>[\d.]+)\s+ms\),\s+"
    r"Load:\s+(?P<load>-?[\d.]+)%$"
)

NETIO_RE = re.compile(
    r"^Network I/O time:\s+(?P<io>[\d.]+)\s+ms\s+\(max:\s+(?P<max>[\d.]+)\s+ms\)$"
)

DURATION_RE = re.compile(r"^(?P<value>[\d.]+)(?P<unit>ns|µs|μs|us|ms|s|m)$")

UNIT_TO_MS = {
    "ns": 1e-6,
    "µs": 1e-3,
    "μs": 1e-3,
    "us": 1e-3,
    "ms": 1.0,
    "s": 1000.0,
    "m": 60_000.0,
}

# ---------------------------------------------------------------------------
# Known measure! nesting, used to compute exclusive (self) time.
# Unknown labels are still reported; they simply have no children.
# ---------------------------------------------------------------------------

CHILDREN = {
    "server.tick(&mut gs)": [
        "self.game_tick(gs)",
        "self.compress_ticks(gs)",
        "self.handle_network_io(gs)",
    ],
    "self.game_tick(gs)": [
        "gs.tick_element_switch_states(ticker)",
        "self.maybe_enqueue_background_save(gs)",
        "weather.area_system_tick",
        "player.tick_and_online_count",
        "player.process_commands_and_idle",
        "player.tick_login_state",
        "player.send_normal_state_updates",
        "character.main_tick",
        "populate::pop_tick(gs)",
        "EffectManager::effect_tick(gs)",
        "driver::item_tick(gs)",
        "crate::aura::logic::tick_auras(gs, ticker)",
        "self.global_tick(gs)",
    ],
    "crate::aura::logic::tick_auras(gs, ticker)": [
        "aura.tick",
    ],
    "aura.tick": [
        "aura.tile_scan",
    ],
    "player.send_normal_state_updates": [
        "player.getmap",
        "player.change",
    ],
}

ROOT_LABEL = "server.tick(&mut gs)"

# `server.tick` wraps the whole scheduling frame *including* the sleep that
# paces the loop to 36 TPS, so it is the wrong denominator for "% of tick".
# Busy time is the sum of the three things the frame actually does.
BUSY_LABELS = [
    "self.game_tick(gs)",
    "self.compress_ticks(gs)",
    "self.handle_network_io(gs)",
]

# Target tick budget: TICK is 36 ticks/sec in core::constants.
TARGET_TICK_MS = 1000.0 / 36.0

# Leaf frames that mean "this thread is parked", not "this thread is working".
IDLE_FRAMES = (
    "nanosleep",
    "semaphore_wait_trap",
    "__semwait_signal",
    "__psynch_cvwait",
    "_pthread_cond_wait",
    "kevent",
    "kevent_id",
    "__workq_kernreturn",
    "mach_msg2_trap",
    "mach_msg_trap",
    "__recvfrom",
    "__select",
    "__accept",
    "__read_nocancel",
    "poll",
    "start_wqthread",
    "thread_start",
    "_pthread_start",
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def parse_duration_ms(text: str) -> float | None:
    """Convert a Rust ``Duration`` debug string into milliseconds.

    # Arguments

    * `text` - A duration rendered by `{:?}`, e.g. `1.25ms` or `430.5µs`.

    # Returns

    * The duration in milliseconds, or `None` if it could not be parsed.
    """
    match = DURATION_RE.match(text.strip())
    if not match:
        return None
    return float(match.group("value")) * UNIT_TO_MS[match.group("unit")]


def parse_timestamp(text: str) -> float:
    """Convert a log timestamp into a UTC epoch float.

    # Arguments

    * `text` - Timestamp in `%Y-%m-%dT%H:%M:%S[.fffffffff]` UTC form.

    # Returns

    * Seconds since the Unix epoch.
    """
    if "." in text:
        head, frac = text.split(".", 1)
        # Python only understands microsecond precision.
        frac = (frac + "000000")[:6]
        text = f"{head}.{frac}"
        fmt = "%Y-%m-%dT%H:%M:%S.%f"
    else:
        fmt = "%Y-%m-%dT%H:%M:%S"
    return datetime.strptime(text, fmt).replace(tzinfo=timezone.utc).timestamp()


def percentile(sorted_values: list[float], pct: float) -> float:
    """Return the ``pct`` percentile of an already-sorted list."""
    if not sorted_values:
        return 0.0
    if len(sorted_values) == 1:
        return sorted_values[0]
    rank = (len(sorted_values) - 1) * (pct / 100.0)
    low = math.floor(rank)
    high = math.ceil(rank)
    if low == high:
        return sorted_values[int(rank)]
    return sorted_values[low] + (sorted_values[high] - sorted_values[low]) * (rank - low)


def perf_log_files(log_dir: Path) -> list[Path]:
    """Collect `server_perf.log` plus its rotated siblings, oldest first."""
    base = log_dir / "server_perf.log"
    rotated = sorted(
        log_dir.glob("server_perf.log.*"),
        key=lambda p: int(p.suffix.lstrip(".")) if p.suffix.lstrip(".").isdigit() else 0,
        reverse=True,
    )
    return [p for p in [*rotated, base] if p.exists()]


# ---------------------------------------------------------------------------
# Aggregation
# ---------------------------------------------------------------------------


class Series:
    """Accumulates samples for a single measurement label."""

    def __init__(self, label: str) -> None:
        self.label = label
        self.samples: list[float] = []

    def add(self, value: float) -> None:
        self.samples.append(value)

    def stats(self) -> dict:
        data = sorted(self.samples)
        total = sum(data)
        count = len(data)
        return {
            "count": count,
            "total_ms": total,
            "mean_ms": total / count if count else 0.0,
            "p50_ms": percentile(data, 50),
            "p95_ms": percentile(data, 95),
            "p99_ms": percentile(data, 99),
            "max_ms": data[-1] if data else 0.0,
        }


def collect(run_dir: Path, window: tuple[float, float] | None) -> dict:
    """Parse every perf log in a run directory into per-label series.

    # Arguments

    * `run_dir` - Directory produced by `perf_loadtest.sh`.
    * `window` - Optional `(start_epoch, end_epoch)` filter.

    # Returns

    * A dict with the measure series, tick/network samples and line counts.
    """
    log_dir = run_dir / "server-logs"
    files = perf_log_files(log_dir)
    if not files:
        raise SystemExit(f"No server_perf.log found under {log_dir}")

    measures: dict[str, Series] = {}
    tick_ms: list[float] = []
    load_pct: list[float] = []
    netio_ms: list[float] = []
    first_ts: float | None = None
    last_ts: float | None = None
    parsed = 0
    skipped = 0

    for path in files:
        with path.open("r", errors="replace") as handle:
            for raw in handle:
                line = LINE_RE.match(raw.rstrip("\n"))
                if not line:
                    continue
                ts = parse_timestamp(line.group("ts"))
                if window and not (window[0] <= ts <= window[1]):
                    skipped += 1
                    continue
                msg = line.group("msg")

                first_ts = ts if first_ts is None else min(first_ts, ts)
                last_ts = ts if last_ts is None else max(last_ts, ts)

                measure = MEASURE_RE.match(msg)
                if measure:
                    value = parse_duration_ms(measure.group("dur"))
                    if value is None:
                        continue
                    label = measure.group("label")
                    measures.setdefault(label, Series(label)).add(value)
                    parsed += 1
                    continue

                tick = TICK_RE.match(msg)
                if tick:
                    tick_ms.append(float(tick.group("tick")))
                    load_pct.append(float(tick.group("load")))
                    parsed += 1
                    continue

                netio = NETIO_RE.match(msg)
                if netio:
                    netio_ms.append(float(netio.group("io")))
                    parsed += 1

    return {
        "measures": measures,
        "tick_ms": tick_ms,
        "load_pct": load_pct,
        "netio_ms": netio_ms,
        "first_ts": first_ts,
        "last_ts": last_ts,
        "parsed": parsed,
        "skipped": skipped,
        "files": [str(p) for p in files],
    }


def exclusive_ms(label: str, stats_by_label: dict[str, dict]) -> float:
    """Total time in ``label`` minus the total time of its known children."""
    own = stats_by_label.get(label, {}).get("total_ms", 0.0)
    for child in CHILDREN.get(label, []):
        own -= stats_by_label.get(child, {}).get("total_ms", 0.0)
    return own


# ---------------------------------------------------------------------------
# Auxiliary artifacts
# ---------------------------------------------------------------------------


def read_resources(run_dir: Path, window: tuple[float, float] | None) -> dict | None:
    """Summarise the CPU/RSS sampler CSV, if present."""
    path = run_dir / "server_resources.csv"
    if not path.exists():
        return None
    cpu: list[float] = []
    rss: list[float] = []
    for line in path.read_text(errors="replace").splitlines()[1:]:
        parts = line.split(",")
        if len(parts) != 3:
            continue
        try:
            ts, cpu_pct, rss_kb = float(parts[0]), float(parts[1]), float(parts[2])
        except ValueError:
            continue
        if window and not (window[0] <= ts <= window[1]):
            continue
        cpu.append(cpu_pct)
        rss.append(rss_kb)
    if not cpu:
        return None
    cpu.sort()
    rss.sort()
    return {
        "samples": len(cpu),
        "cpu_mean_pct": sum(cpu) / len(cpu),
        "cpu_p95_pct": percentile(cpu, 95),
        "cpu_max_pct": cpu[-1],
        "rss_mean_mb": (sum(rss) / len(rss)) / 1024.0,
        "rss_max_mb": rss[-1] / 1024.0,
    }


SAMPLE_FRAME_RE = re.compile(r"^(?P<prefix>[\s+!:|]*)(?P<count>\d+)\s+(?P<rest>.+)$")

THREAD_RE = re.compile(r"^\s*(?P<count>\d+)\s+(?P<thread>Thread_\S+)(?P<rest>.*)$")

# `Cs<base62>_` crate disambiguators and `B<n>_` backrefs are v0 noise that
# the length-prefix scanner would otherwise turn into junk path components.
V0_NOISE_RE = re.compile(r"C[su][0-9A-Za-z]*_|B[0-9A-Za-z]*_")


def normalize_frame(rest: str) -> str:
    """Strip image, offset and address decorations from a ``sample`` frame.

    # Arguments

    * `rest` - Everything after the sample count on a call-graph line.

    # Returns

    * The bare symbol name.
    """
    for marker in ("  (in ", " (in "):
        if marker in rest:
            rest = rest.split(marker, 1)[0]
            break
    rest = re.sub(r"\s+\[0x[0-9a-f]+\].*$", "", rest)
    rest = re.sub(r"\s+\+\s+[\d,.]+.*$", "", rest)
    return rest.strip()


def demangle(symbol: str) -> str:
    """Best-effort readable form of a Rust symbol name.

    Handles legacy `_ZN..E` and Rust v0 `_R..` mangling well enough for
    reporting. Non-Rust symbols are returned unchanged.

    # Arguments

    * `symbol` - A possibly-mangled symbol name.

    # Returns

    * A `crate::module::item` style string, or the original symbol.
    """
    if not symbol.startswith(("_R", "_ZN", "__ZN")):
        return symbol

    body = V0_NOISE_RE.sub("", symbol)
    parts: list[str] = []
    index = 0
    length = len(body)
    while index < length:
        if not body[index].isdigit():
            index += 1
            continue
        end = index
        while end < length and body[end].isdigit():
            end += 1
        size = int(body[index:end])
        name = body[end : end + size]
        if len(name) == size and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", name):
            # 17h<hex> is the legacy-mangling hash suffix; drop it.
            if not re.fullmatch(r"h[0-9a-f]{16}", name):
                parts.append(name)
            index = end + size
        else:
            index = end

    if not parts:
        return symbol
    # Collapse repeated adjacent components (`server::server::Server`).
    collapsed = [parts[0]]
    for part in parts[1:]:
        if part != collapsed[-1]:
            collapsed.append(part)
    return "::".join(collapsed)


def read_sample_threads(run_dir: Path, top: int = 20) -> list[dict] | None:
    """Summarise a macOS ``sample`` capture, one entry per thread.

    Self time per frame is ``count - sum(direct children counts)``, using the
    call-graph's `+ ! : |` prefix characters as the depth indicator.

    # Arguments

    * `run_dir` - Run directory containing `sample.txt`.
    * `top` - Maximum number of frames reported per thread.

    # Returns

    * A list of thread summaries sorted by busy samples, or `None`.
    """
    path = run_dir / "sample.txt"
    if not path.exists():
        return None

    lines = path.read_text(errors="replace").splitlines()
    try:
        start = next(i for i, line in enumerate(lines) if line.startswith("Call graph:"))
    except StopIteration:
        return None
    end = next(
        (
            i
            for i, line in enumerate(lines)
            if line.startswith(
                ("Total number in stack", "Sort by top of stack", "Binary Images:")
            )
        ),
        len(lines),
    )

    threads: list[dict] = []
    entries: list[tuple[int, int, str]] = []

    def flush() -> None:
        if not threads or not entries:
            return
        self_time: dict[str, int] = {}
        idle = 0
        for idx, (depth, count, frame) in enumerate(entries):
            child_total = 0
            for child_depth, child_count, _ in entries[idx + 1 :]:
                if child_depth <= depth:
                    break
                if child_depth == depth + 2:
                    child_total += child_count
            own = max(count - child_total, 0)
            if own == 0:
                continue
            if frame in IDLE_FRAMES:
                idle += own
                continue
            pretty = demangle(frame)
            self_time[pretty] = self_time.get(pretty, 0) + own
        ranked = sorted(self_time.items(), key=lambda kv: kv[1], reverse=True)
        threads[-1]["idle_samples"] = idle
        threads[-1]["busy_samples"] = sum(self_time.values())
        threads[-1]["frames"] = ranked[:top]
        entries.clear()

    for raw in lines[start + 1 : end]:
        if not raw.strip():
            continue
        thread = THREAD_RE.match(raw)
        if thread:
            flush()
            threads.append(
                {
                    "name": thread.group("thread"),
                    "label": thread.group("rest").strip(),
                    "total_samples": int(thread.group("count")),
                    "idle_samples": 0,
                    "busy_samples": 0,
                    "frames": [],
                }
            )
            continue
        match = SAMPLE_FRAME_RE.match(raw)
        if not match:
            continue
        frame = normalize_frame(match.group("rest"))
        if not frame:
            continue
        entries.append((len(match.group("prefix")), int(match.group("count")), frame))
    flush()

    threads.sort(key=lambda t: t["busy_samples"], reverse=True)
    return [t for t in threads if t["total_samples"] > 0]


def read_loadtest_summary(run_dir: Path, tail: int = 40) -> list[str] | None:
    """Return the tail of the load-test log (its final metrics report)."""
    path = run_dir / "loadtest.log"
    if not path.exists():
        return None
    return path.read_text(errors="replace").splitlines()[-tail:]


# ---------------------------------------------------------------------------
# Reporting
# ---------------------------------------------------------------------------


def fmt_row(cells: list[str], widths: list[int]) -> str:
    return "  ".join(cell.ljust(width) for cell, width in zip(cells, widths)).rstrip()


def render_table(headers: list[str], rows: list[list[str]], max_width: int = 78) -> list[str]:
    rows = [[cell if len(cell) <= max_width else cell[: max_width - 1] + "…" for cell in row]
            for row in rows]
    widths = [len(h) for h in headers]
    for row in rows:
        for i, cell in enumerate(row):
            widths[i] = max(widths[i], len(cell))
    out = [fmt_row(headers, widths), fmt_row(["-" * w for w in widths], widths)]
    out.extend(fmt_row(row, widths) for row in rows)
    return out


def build_report(run_dir: Path, meta: dict, data: dict) -> str:
    measures: dict[str, Series] = data["measures"]
    stats_by_label = {label: series.stats() for label, series in measures.items()}

    lines: list[str] = []
    add = lines.append

    add(f"# Server performance report — {run_dir.name}")
    add("")
    add("## Run parameters")
    add("")
    if meta:
        for key in (
            "clients",
            "duration_secs",
            "ramp_up_secs",
            "login_stagger_secs",
            "cargo_profile",
            "loadtest_exit_code",
        ):
            if key in meta:
                add(f"- {key}: {meta[key]}")
    window_secs = 0.0
    if data["first_ts"] is not None and data["last_ts"] is not None:
        window_secs = data["last_ts"] - data["first_ts"]
        add(f"- analysis window: {window_secs:.0f}s of steady-state traffic")
    add(f"- perf log lines analysed: {data['parsed']:,} (outside window: {data['skipped']:,})")
    add("")

    # --- Tick health ------------------------------------------------------
    tick_ms = sorted(data["tick_ms"])
    if tick_ms:
        add("## Tick health")
        add("")
        add(f"- target tick budget: {TARGET_TICK_MS:.2f} ms (36 TPS)")
        add(f"- tick time mean: {sum(tick_ms) / len(tick_ms):.2f} ms")
        add(f"- tick time p50/p95/p99: {percentile(tick_ms, 50):.2f} / "
            f"{percentile(tick_ms, 95):.2f} / {percentile(tick_ms, 99):.2f} ms")
        add(f"- tick time max: {tick_ms[-1]:.2f} ms")
        over = sum(1 for v in tick_ms if v > TARGET_TICK_MS)
        add(f"- ticks over budget: {over}/{len(tick_ms)} ({100.0 * over / len(tick_ms):.1f}%)")
        if data["load_pct"]:
            loads = sorted(data["load_pct"])
            add(f"- reported load p50/p95/max: {percentile(loads, 50):.1f}% / "
                f"{percentile(loads, 95):.1f}% / {loads[-1]:.1f}%")
        add("")

    netio = sorted(data["netio_ms"])
    if netio:
        add("## Network I/O")
        add("")
        add(f"- mean: {sum(netio) / len(netio):.2f} ms, p95: {percentile(netio, 95):.2f} ms, "
            f"max: {netio[-1]:.2f} ms")
        add("")

    # --- Inclusive cost ---------------------------------------------------
    frame_total = stats_by_label.get(ROOT_LABEL, {}).get("total_ms", 0.0)
    busy_total = sum(
        stats_by_label.get(label, {}).get("total_ms", 0.0) for label in BUSY_LABELS
    )

    if frame_total > 0.0 and busy_total > 0.0:
        add("## Headroom")
        add("")
        add(f"- scheduling frames measured: {stats_by_label[ROOT_LABEL]['count']:,}")
        add(f"- busy time: {busy_total / 1000.0:.1f}s of {frame_total / 1000.0:.1f}s wall "
            f"({100.0 * busy_total / frame_total:.1f}% of one core)")
        add(f"- idle (tick-pacing sleep): {100.0 * (1 - busy_total / frame_total):.1f}%")
        add(f"- mean busy time per frame: {busy_total / stats_by_label[ROOT_LABEL]['count']:.2f} ms "
            f"against a {TARGET_TICK_MS:.2f} ms budget")
        add("")

    denominator = busy_total if busy_total > 0.0 else frame_total

    add("## Where busy time goes (inclusive)")
    add("")
    rows = []
    for label, stats in sorted(
        stats_by_label.items(), key=lambda kv: kv[1]["total_ms"], reverse=True
    ):
        if label == ROOT_LABEL:
            continue
        share = (100.0 * stats["total_ms"] / denominator) if denominator else 0.0
        rows.append(
            [
                label,
                f"{stats['count']:,}",
                f"{stats['total_ms'] / 1000.0:.2f}",
                f"{share:.1f}%",
                f"{stats['mean_ms']:.3f}",
                f"{stats['p95_ms']:.3f}",
                f"{stats['p99_ms']:.3f}",
                f"{stats['max_ms']:.2f}",
            ]
        )
    lines.extend(
        render_table(
            ["label", "calls", "total s", "% busy", "mean ms", "p95 ms", "p99 ms", "max ms"],
            rows,
        )
    )
    add("")

    # --- Exclusive cost ---------------------------------------------------
    add("## Hot spots (exclusive / self time)")
    add("")
    excl_rows = []
    for label in stats_by_label:
        if label == ROOT_LABEL:
            continue
        own = exclusive_ms(label, stats_by_label)
        share = (100.0 * own / denominator) if denominator else 0.0
        excl_rows.append((own, share, label))
    excl_rows.sort(reverse=True)
    lines.extend(
        render_table(
            ["label", "self s", "% busy"],
            [
                [label, f"{own / 1000.0:.2f}", f"{share:.1f}%"]
                for own, share, label in excl_rows
            ],
        )
    )
    add("")
    add("Note: `self` time for a parent includes any un-instrumented work inside it,")
    add("so a large self value is an invitation to add finer `measure!` sites there.")
    add("")

    # --- Process resources ------------------------------------------------
    resources = data.get("resources")
    if resources:
        add("## Server process resources (steady state)")
        add("")
        add(f"- CPU mean: {resources['cpu_mean_pct']:.1f}% of one core "
            f"(p95 {resources['cpu_p95_pct']:.1f}%, max {resources['cpu_max_pct']:.1f}%)")
        add(f"- RSS mean: {resources['rss_mean_mb']:.0f} MB (max {resources['rss_max_mb']:.0f} MB)")
        add("")

    # --- Sampled call tree ------------------------------------------------
    threads = data.get("sample_threads")
    if threads:
        add("## Sampled self time (macOS `sample`)")
        add("")
        for thread in threads[:4]:
            if thread["busy_samples"] == 0:
                continue
            add(f"### {thread['name']} {thread['label']}".rstrip())
            add("")
            add(f"- {thread['total_samples']:,} samples "
                f"({thread['idle_samples']:,} parked, {thread['busy_samples']:,} busy)")
            add("")
            busy = thread["busy_samples"]
            lines.extend(
                render_table(
                    ["frame", "self samples", "% busy"],
                    [
                        [frame, str(count), f"{100.0 * count / busy:.1f}%"]
                        for frame, count in thread["frames"]
                    ],
                )
            )
            add("")

    # --- Client view ------------------------------------------------------
    summary = data.get("loadtest_summary")
    if summary:
        add("## Load-test client summary (tail)")
        add("")
        add("```")
        lines.extend(summary)
        add("```")
        add("")

    return "\n".join(lines) + "\n"


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-dir", required=True, help="Run directory to analyze.")
    parser.add_argument(
        "--full-window",
        action="store_true",
        help="Analyze the whole log instead of only the steady-state window.",
    )
    args = parser.parse_args()

    run_dir = Path(args.run_dir)
    if not run_dir.is_dir():
        raise SystemExit(f"Run directory not found: {run_dir}")

    meta_path = run_dir / "run_meta.json"
    meta = json.loads(meta_path.read_text()) if meta_path.exists() else {}

    window = None
    if not args.full_window and "steady_start_utc" in meta:
        window = (float(meta["steady_start_utc"]), float(meta["steady_end_utc"]))

    data = collect(run_dir, window)
    if not data["measures"] and not data["tick_ms"]:
        raise SystemExit(
            "No perf samples found in the selected window. "
            "Re-run with --full-window, or confirm the server was built "
            "with --features measure-time."
        )

    data["resources"] = read_resources(run_dir, window)
    data["sample_threads"] = read_sample_threads(run_dir)
    data["loadtest_summary"] = read_loadtest_summary(run_dir)

    report = build_report(run_dir, meta, data)
    (run_dir / "perf_report.md").write_text(report)

    json_summary = {
        "run_dir": str(run_dir),
        "meta": meta,
        "measures": {label: series.stats() for label, series in data["measures"].items()},
        "tick_ms_count": len(data["tick_ms"]),
        "resources": data["resources"],
    }
    (run_dir / "perf_summary.json").write_text(json.dumps(json_summary, indent=2))

    sys.stdout.write(report)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
