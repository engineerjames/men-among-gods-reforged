//! Report module: prints human-readable benchmark results and pass/fail verdict.

use crate::tick_sim::{TickStats, NUM_PHASES, PHASE_NAMES};
use std::time::Duration;

/// Target tick budget: 1,000,000µs / 36 TPS ≈ 27,778µs.
pub const TARGET_TICK_US: f64 = 1_000_000.0 / 36.0;

/// Results from a benchmark run of one schema variant.
#[derive(Debug, Clone)]
pub struct SchemaResult {
    pub schema_name: String,
    pub population_label: String,
    pub tick_durations: Vec<Duration>,
    /// Accumulated phase durations across all sampled ticks.
    pub phase_totals: [Duration; NUM_PHASES],
    /// Number of samples (for computing phase means).
    pub sample_count: usize,
}

impl SchemaResult {
    pub fn new(schema_name: &str, population_label: &str) -> Self {
        Self {
            schema_name: schema_name.to_string(),
            population_label: population_label.to_string(),
            tick_durations: Vec::new(),
            phase_totals: [Duration::ZERO; NUM_PHASES],
            sample_count: 0,
        }
    }

    pub fn add_sample_with_stats(&mut self, total: Duration, stats: &TickStats) {
        self.tick_durations.push(total);
        for (i, &d) in stats.phase_durations.iter().enumerate() {
            self.phase_totals[i] += d;
        }
        self.sample_count += 1;
    }

    /// Mean phase duration in microseconds.
    pub fn phase_mean_us(&self, phase: usize) -> f64 {
        if self.sample_count == 0 {
            return 0.0;
        }
        self.phase_totals[phase].as_secs_f64() * 1e6 / self.sample_count as f64
    }

    /// Phase as percentage of total mean tick time.
    pub fn phase_pct(&self, phase: usize) -> f64 {
        let total = self.mean_us();
        if total <= 0.0 {
            return 0.0;
        }
        self.phase_mean_us(phase) / total * 100.0
    }

    pub fn mean_us(&self) -> f64 {
        if self.tick_durations.is_empty() {
            return 0.0;
        }
        let sum: f64 = self
            .tick_durations
            .iter()
            .map(|d| d.as_secs_f64() * 1e6)
            .sum();
        sum / self.tick_durations.len() as f64
    }

    pub fn percentile_us(&self, pct: f64) -> f64 {
        if self.tick_durations.is_empty() {
            return 0.0;
        }
        let mut sorted: Vec<f64> = self
            .tick_durations
            .iter()
            .map(|d| d.as_secs_f64() * 1e6)
            .collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((pct / 100.0) * (sorted.len() - 1) as f64).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn max_sustainable_tps(&self) -> f64 {
        let mean = self.mean_us();
        if mean <= 0.0 {
            return 0.0;
        }
        1_000_000.0 / mean
    }

    pub fn passes_budget(&self) -> bool {
        self.mean_us() < TARGET_TICK_US
    }
}

/// Print a formatted report comparing schema results.
pub fn print_report(results: &[SchemaResult]) {
    println!("\n{}", "=".repeat(80));
    println!("  SQLite In-Memory Benchmark Report");
    println!(
        "  Target: 36 TPS ({:.0}µs / {:.2}ms per tick)",
        TARGET_TICK_US,
        TARGET_TICK_US / 1000.0
    );
    println!("{}", "=".repeat(80));

    for result in results {
        let mean = result.mean_us();
        let p50 = result.percentile_us(50.0);
        let p95 = result.percentile_us(95.0);
        let p99 = result.percentile_us(99.0);
        let max_tps = result.max_sustainable_tps();
        let verdict = if result.passes_budget() {
            "PASS"
        } else {
            "FAIL"
        };
        let headroom = TARGET_TICK_US - mean;

        println!(
            "\n  Schema: {} | Population: {}",
            result.schema_name, result.population_label
        );
        println!("  {}", "-".repeat(60));
        println!(
            "  Mean tick:       {:>10.0}µs  ({:.2}ms)",
            mean,
            mean / 1000.0
        );
        println!("  p50:             {:>10.0}µs", p50);
        println!("  p95:             {:>10.0}µs", p95);
        println!("  p99:             {:>10.0}µs", p99);
        println!("  Max sustained:   {:>10.1} TPS", max_tps);
        println!(
            "  Headroom:        {:>+10.0}µs  ({:>+.2}ms)",
            headroom,
            headroom / 1000.0
        );
        println!("  Verdict:         {}", verdict);

        // Per-phase breakdown
        println!("\n  Phase breakdown (mean per tick):");
        println!(
            "  {:24} {:>10} {:>8} {:>6}",
            "Phase", "Mean (µs)", "Mean (ms)", "% tick"
        );
        println!("  {}", "-".repeat(52));

        // Sort phases by duration descending for easier reading
        let mut phase_indices: Vec<usize> = (0..NUM_PHASES).collect();
        phase_indices.sort_by(|&a, &b| {
            result
                .phase_mean_us(b)
                .partial_cmp(&result.phase_mean_us(a))
                .unwrap()
        });

        for &pi in &phase_indices {
            let pmu = result.phase_mean_us(pi);
            let pct = result.phase_pct(pi);
            if pmu < 0.1 {
                continue; // skip negligible phases
            }
            println!(
                "  {:24} {:>10.0} {:>8.2} {:>5.1}%",
                PHASE_NAMES[pi],
                pmu,
                pmu / 1000.0,
                pct,
            );
        }
    }

    println!("\n{}", "=".repeat(80));

    // Comparison table
    if results.len() >= 2 {
        println!("\n  Comparison Summary:");
        println!(
            "  {:20} {:>12} {:>12} {:>10} {:>8}",
            "Schema / Pop", "Mean (µs)", "p95 (µs)", "Max TPS", "Pass?"
        );
        println!("  {}", "-".repeat(66));
        for r in results {
            let label = format!("{}/{}", r.schema_name, r.population_label);
            println!(
                "  {:20} {:>12.0} {:>12.0} {:>10.1} {:>8}",
                label,
                r.mean_us(),
                r.percentile_us(95.0),
                r.max_sustainable_tps(),
                if r.passes_budget() { "YES" } else { "NO" }
            );
        }
    }

    println!();
}
