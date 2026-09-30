#!/usr/bin/env python3
"""Analyze a `perf_client.sh` run directory and report client frame-time hot spots.

Reads the artifacts produced by ``scripts/perf_client.sh``:

* ``client-data/mag_client_perf.log`` (plus rotated siblings) — per-frame
  ``measure!`` rows written by the client main loop (``client.frame``,
  ``client.handle_events``, ``client.update``, ``client.render_world``,
  ``client.present``).
* ``client-data/mag_client.log`` — scene transitions, automation milestones
  and the in-game render profiler summary (``=== Performance Profile ...``).
* ``client_resources.csv`` — CPU / RSS samples of the client process.
* ``sample.txt`` — optional macOS ``sample`` call-tree capture.
* ``run_meta.json`` — run parameters written by the harness.

By default the analysis window is the render profiler's capture window (the
steady-state, in-world part of the session). ``--full-window`` analyses the
whole log instead, which also covers login and character creation.

Shares its log grammar and helpers with ``analyze_perf_log.py``; only the
Python standard library is used.

Usage:
    python3 scripts/analyze_client_perf.py --run-dir perf-runs/client-2026-09-29_10-00-00
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from analyze_perf_log import (  # noqa: E402
    LINE_RE,
    MEASURE_RE,
    Series,
    parse_duration_ms,
    parse_timestamp,
    percentile,
    read_resources,
    read_sample_threads,
    render_table,
)

# Frame budget at the client's 60 FPS cap (menus) and at the 36 TPS server
# tick the game scene paces its presentation to (LegacyTickScheduler).
TARGET_FRAME_MS = 1000.0 / 60.0
TICK_FRAME_MS = 1000.0 / 36.0

# Per-frame sections in the order they run inside one main-loop iteration.
FRAME_LABEL = "client.frame"
SECTION_LABELS = [
    "client.handle_events",
    "client.update",
    "client.render_world",
    "client.present",
]

PROFILE_START_RE = re.compile(r"^Performance profiling started \((?P<secs>\d+)s window\)$")
PROFILE_BLOCK_START = "=== Performance Profile"
PROFILE_BLOCK_END = "=== End Performance Profile ==="
SCENE_SWITCH_RE = re.compile(r"^Switching to scene: (?P<scene>\w+)$")
AUTOMATION_RE = re.compile(r"^Automation(?: script)?[: ](?P<msg>.*)$")


# ---------------------------------------------------------------------------
# Log discovery
# ---------------------------------------------------------------------------


def rotated_log_files(base: Path) -> list[Path]:
    """Return `base` plus its rotated `.N` siblings, oldest first."""
    rotated = sorted(
        base.parent.glob(f"{base.name}.*"),
        key=lambda p: int(p.suffix.lstrip(".")) if p.suffix.lstrip(".").isdigit() else 0,
        reverse=True,
    )
    return [p for p in [*rotated, base] if p.exists()]


def iter_log_lines(files: list[Path]):
    """Yield `(epoch, message)` for every parseable line across `files`."""
    for path in files:
        with path.open("r", errors="replace") as handle:
            for raw in handle:
                match = LINE_RE.match(raw.rstrip("\n"))
                if not match:
                    continue
                yield parse_timestamp(match.group("ts")), match.group("msg")


# ---------------------------------------------------------------------------
# Client log: timeline, profiler window, profiler summary
# ---------------------------------------------------------------------------


def read_client_log(run_dir: Path) -> dict:
    """Extract milestones, the profiler window and profiler summaries.

    # Arguments

    * `run_dir` - Run directory containing `client-data/mag_client.log`.

    # Returns

    * A dict with `timeline`, `profile_window`, `profile_blocks`, `errors`
      and `first_ts`.
    """
    files = rotated_log_files(run_dir / "client-data" / "mag_client.log")
    timeline: list[tuple[float, str]] = []
    profile_blocks: list[list[str]] = []
    errors: list[str] = []
    profile_start: float | None = None
    profile_end: float | None = None
    first_ts: float | None = None
    current_block: list[str] | None = None

    for ts, msg in iter_log_lines(files):
        first_ts = ts if first_ts is None else first_ts

        if current_block is not None:
            current_block.append(msg)
            if msg.startswith(PROFILE_BLOCK_END):
                profile_blocks.append(current_block)
                current_block = None
                if profile_end is None:
                    profile_end = ts
            continue

        scene = SCENE_SWITCH_RE.match(msg)
        if scene:
            timeline.append((ts, f"scene -> {scene.group('scene')}"))
            continue

        if PROFILE_START_RE.match(msg):
            if profile_start is None:
                profile_start = ts
            timeline.append((ts, msg))
            continue

        if msg.startswith(PROFILE_BLOCK_START):
            current_block = [msg]
            continue

        automation = AUTOMATION_RE.match(msg)
        if automation:
            text = automation.group("msg").strip()
            timeline.append((ts, f"automation: {text}"))
            if "failed" in msg.lower():
                errors.append(msg)
            continue

        if msg.startswith(("Login failed", "Failed to", "Connection")):
            errors.append(msg)

    window = None
    if profile_start is not None:
        window = (profile_start, profile_end if profile_end is not None else profile_start + 3600)

    return {
        "timeline": timeline,
        "profile_window": window,
        "profile_blocks": profile_blocks,
        "errors": errors[:20],
        "first_ts": first_ts,
    }


# ---------------------------------------------------------------------------
# Perf log: measure! rows
# ---------------------------------------------------------------------------


def collect_measures(run_dir: Path, window: tuple[float, float] | None) -> dict:
    """Aggregate `[measure-time]` rows from the client perf log.

    # Arguments

    * `run_dir` - Run directory.
    * `window` - Optional `(start_epoch, end_epoch)` filter.

    # Returns

    * A dict with per-label `Series`, the raw frame samples and counts.
    """
    files = rotated_log_files(run_dir / "client-data" / "mag_client_perf.log")
    measures: dict[str, Series] = {}
    parsed = 0
    skipped = 0
    first_ts: float | None = None
    last_ts: float | None = None

    for ts, msg in iter_log_lines(files):
        measure = MEASURE_RE.match(msg)
        if not measure:
            continue
        if window and not (window[0] <= ts <= window[1]):
            skipped += 1
            continue
        value = parse_duration_ms(measure.group("dur"))
        if value is None:
            continue
        first_ts = ts if first_ts is None else min(first_ts, ts)
        last_ts = ts if last_ts is None else max(last_ts, ts)
        measures.setdefault(measure.group("label"), Series(measure.group("label"))).add(value)
        parsed += 1

    return {
        "measures": measures,
        "parsed": parsed,
        "skipped": skipped,
        "first_ts": first_ts,
        "last_ts": last_ts,
        "files": [str(p) for p in files],
    }


def frame_stats(series: Series | None) -> dict | None:
    """Frame-time distribution plus derived FPS and over-budget counts."""
    if series is None or not series.samples:
        return None
    stats = series.stats()
    over = sum(1 for s in series.samples if s > TARGET_FRAME_MS)
    over_tick = sum(1 for s in series.samples if s > TICK_FRAME_MS * 1.5)
    stats["over_budget"] = over
    stats["over_budget_pct"] = 100.0 * over / stats["count"]
    stats["over_tick_budget"] = over_tick
    stats["over_tick_budget_pct"] = 100.0 * over_tick / stats["count"]
    stats["fps_mean"] = 1000.0 / stats["mean_ms"] if stats["mean_ms"] else 0.0
    stats["fps_p95_floor"] = 1000.0 / stats["p95_ms"] if stats["p95_ms"] else 0.0
    return stats


# ---------------------------------------------------------------------------
# Report
# ---------------------------------------------------------------------------


def fmt_epoch(ts: float | None) -> str:
    if ts is None:
        return "-"
    return datetime.fromtimestamp(ts, tz=timezone.utc).strftime("%H:%M:%S")


def build_report(run_dir: Path, meta: dict, data: dict, window_label: str) -> str:
    """Render the markdown report."""
    lines: list[str] = []
    add = lines.append

    measures: dict[str, Series] = data["measures"]
    stats_by_label = {label: series.stats() for label, series in measures.items()}
    frame = frame_stats(measures.get(FRAME_LABEL))
    window = data["window"]

    add(f"# Client performance report — {run_dir.name}")
    add("")
    add(f"- Script: `{meta.get('script', '?')}`")
    add(f"- Account `{meta.get('username', '?')}` / character `{meta.get('character', '?')}`"
        f" (existing account: {bool(meta.get('existing_account'))})")
    add(f"- Settle {meta.get('settle_secs', '?')}s, profile {meta.get('profile_secs', '?')}s,"
        f" cargo profile `{meta.get('cargo_profile', '?')}`")
    add(f"- Client exit code: {meta.get('client_exit_code', '?')}")
    if window:
        add(f"- Analysis window: {window_label} "
            f"({fmt_epoch(window[0])} – {fmt_epoch(window[1])} UTC, "
            f"{window[1] - window[0]:.0f}s)")
    else:
        add(f"- Analysis window: {window_label}")
    add(f"- measure! rows parsed: {data['parsed']:,} (outside window: {data['skipped']:,})")
    add("")

    # --- Errors -----------------------------------------------------------
    if data["client_log"]["errors"]:
        add("## Errors seen in the client log")
        add("")
        for err in data["client_log"]["errors"]:
            add(f"- {err}")
        add("")

    # --- Timeline ---------------------------------------------------------
    timeline = data["client_log"]["timeline"]
    if timeline:
        add("## Session timeline")
        add("")
        t0 = data["client_log"]["first_ts"] or timeline[0][0]
        rows = [[f"T+{ts - t0:7.1f}s", fmt_epoch(ts), text] for ts, text in timeline]
        lines.extend(render_table(["offset", "utc", "event"], rows, max_width=90))
        add("")

    # --- Frame time --------------------------------------------------------
    add("## Frame time")
    add("")
    if frame is None:
        add("_No `client.frame` rows in the window — was the client built with "
            "`--features measure-time`?_")
        add("")
    else:
        add(f"- {frame['count']:,} frames, mean {frame['mean_ms']:.2f} ms "
            f"(~{frame['fps_mean']:.1f} FPS), p50 {frame['p50_ms']:.2f} ms, "
            f"p95 {frame['p95_ms']:.2f} ms, p99 {frame['p99_ms']:.2f} ms, "
            f"max {frame['max_ms']:.2f} ms")
        add(f"- Budget {TARGET_FRAME_MS:.2f} ms (60 FPS): {frame['over_budget']:,} frames "
            f"over budget ({frame['over_budget_pct']:.1f}%)")
        add(f"- In-game frames are paced to the {TICK_FRAME_MS:.2f} ms server tick (36 TPS), so "
            f"~{TICK_FRAME_MS:.1f} ms is expected there; frames slower than 1.5 ticks: "
            f"{frame['over_tick_budget']:,} ({frame['over_tick_budget_pct']:.1f}%)")
        add("")
        add("`client.frame` is wall time between consecutive loop iterations and "
            "therefore includes the FPS-cap sleep, tick-pacing wait and vsync; the "
            "sections below are the work inside one iteration.")
        add("")

    # --- Section breakdown -------------------------------------------------
    add("## Main-loop sections")
    add("")
    frame_total = stats_by_label.get(FRAME_LABEL, {}).get("total_ms", 0.0)
    rows = []
    labels = [l for l in SECTION_LABELS if l in stats_by_label] + sorted(
        l for l in stats_by_label if l not in SECTION_LABELS and l != FRAME_LABEL
    )
    for label in labels:
        s = stats_by_label[label]
        share = 100.0 * s["total_ms"] / frame_total if frame_total else 0.0
        rows.append(
            [
                label,
                f"{s['count']:,}",
                f"{s['mean_ms']:.3f}",
                f"{s['p50_ms']:.3f}",
                f"{s['p95_ms']:.3f}",
                f"{s['p99_ms']:.3f}",
                f"{s['max_ms']:.2f}",
                f"{share:5.1f}%",
            ]
        )
    if rows:
        lines.extend(
            render_table(
                ["section", "samples", "mean ms", "p50 ms", "p95 ms", "p99 ms", "max ms", "% frame"],
                rows,
            )
        )
        add("")
        add("`client.present` includes the vsync wait when vsync is enabled; a large "
            "share there is idle time, not work.")
    else:
        add("_No section rows found._")
    add("")

    # --- Render profiler ---------------------------------------------------
    blocks = data["client_log"]["profile_blocks"]
    if blocks:
        add("## In-game render profiler")
        add("")
        add("Per-draw-call breakdown captured by `GameScene`'s `PerfProfiler` "
            "(started by the script's `profile` command).")
        add("")
        for block in blocks:
            add("```")
            lines.extend(block)
            add("```")
            add("")
    else:
        add("## In-game render profiler")
        add("")
        add("_No profiler summary found — the script never reached `profile`, "
            "or the client quit before the window elapsed._")
        add("")

    # --- Resources ---------------------------------------------------------
    resources = data["resources"]
    add("## Client process")
    add("")
    if resources:
        add(f"- CPU: mean {resources['cpu_mean_pct']:.1f}%, p95 {resources['cpu_p95_pct']:.1f}%, "
            f"max {resources['cpu_max_pct']:.1f}% ({resources['samples']} samples)")
        add(f"- RSS: mean {resources['rss_mean_mb']:.0f} MB, max {resources['rss_max_mb']:.0f} MB")
    else:
        add("_No resource samples in the window._")
    add("")

    # --- sample(1) ---------------------------------------------------------
    threads = data["sample_threads"]
    if threads:
        add("## Call-tree sample (macOS `sample`)")
        add("")
        add("Self time per frame, busiest threads first; parked frames are excluded.")
        add("")
        for thread in threads[:3]:
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
                        [frame_name, str(count), f"{100.0 * count / busy:.1f}%"]
                        for frame_name, count in thread["frames"]
                    ],
                )
            )
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
        help="Analyze the whole session instead of only the render-profiler window.",
    )
    args = parser.parse_args()

    run_dir = Path(args.run_dir)
    if not run_dir.is_dir():
        raise SystemExit(f"Run directory not found: {run_dir}")

    meta_path = run_dir / "run_meta.json"
    meta = json.loads(meta_path.read_text()) if meta_path.exists() else {}

    client_log = read_client_log(run_dir)

    window = None
    window_label = "full session"
    if not args.full_window and client_log["profile_window"]:
        window = client_log["profile_window"]
        window_label = "render-profiler window"

    data = collect_measures(run_dir, window)
    if not data["measures"] and window is not None:
        # Perf log may be empty inside the window (e.g. profiler never ran to
        # completion); fall back so the report still says something useful.
        data = collect_measures(run_dir, None)
        window = None
        window_label = "full session (no rows inside the profiler window)"
    data["window"] = window
    data["client_log"] = client_log
    data["resources"] = read_resources(run_dir, window, filename="client_resources.csv")
    data["sample_threads"] = read_sample_threads(run_dir)

    report = build_report(run_dir, meta, data, window_label)
    (run_dir / "perf_report.md").write_text(report)

    summary = {
        "run_dir": str(run_dir),
        "meta": meta,
        "window": window,
        "measures": {label: series.stats() for label, series in data["measures"].items()},
        "frame": frame_stats(data["measures"].get(FRAME_LABEL)),
        "resources": data["resources"],
        "timeline": [[ts, text] for ts, text in client_log["timeline"]],
        "errors": client_log["errors"],
    }
    (run_dir / "perf_summary.json").write_text(json.dumps(summary, indent=2))

    sys.stdout.write(report)
    if not data["measures"] and not client_log["profile_blocks"]:
        sys.stderr.write(
            "warning: no perf samples and no profiler summary found; confirm the client "
            "was built with --features measure-time and the script reached the game.\n"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
