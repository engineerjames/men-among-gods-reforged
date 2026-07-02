//! Lock-free metrics store aggregated across all simulated clients.

use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;

/// Thread-safe counters and samples collected during a load-test run.
pub struct Metrics {
    /// Clients that successfully completed the game handshake.
    pub connected: AtomicU64,
    /// Clients that failed to connect or bootstrap.
    pub connect_errors: AtomicU64,
    /// Clients that cleanly disconnected after the run.
    pub disconnects: AtomicU64,
    /// Total inbound bytes received across all clients.
    pub bytes_in: AtomicU64,
    /// Total outbound bytes sent across all clients.
    pub bytes_out: AtomicU64,
    /// Total framed tick packets received (one per server tick, per client).
    pub ticks_total: AtomicU64,
    /// Count of inter-tick gaps > 100 ms (possible server slowdown indicator).
    pub tick_gap_late: AtomicU64,
    /// Total outbound command packets sent (move + ping + CTick).
    pub pkts_out: AtomicU64,
    /// Sum of per-client actual connected durations in milliseconds.
    ///
    /// Accumulated at disconnect time so the final report can compute ticks/s
    /// relative to actual connection time rather than wall-clock run duration.
    pub total_client_connected_ms: AtomicU64,
    /// Clients that completed the one-shot login dispersion sequence (said
    /// the god password and sent `/goto`) successfully.
    pub dispersion_sent: AtomicU64,
    /// Clients whose login dispersion sequence failed to send.
    pub dispersion_errors: AtomicU64,
    /// Total periodic slash-commands (from `[[commands]]`) sent successfully.
    pub commands_sent: AtomicU64,
    /// Total periodic slash-commands that failed to send.
    pub commands_errors: AtomicU64,
    /// Collected RTT samples in milliseconds, from CL_PING / SV_PONG exchanges.
    pub rtt_samples: Mutex<Vec<u32>>,
}

impl Metrics {
    /// Creates a zeroed metrics store.
    ///
    /// # Returns
    ///
    /// * A new [`Metrics`] with all counters at zero and no samples.
    pub fn new() -> Self {
        Self {
            connected: AtomicU64::new(0),
            connect_errors: AtomicU64::new(0),
            disconnects: AtomicU64::new(0),
            bytes_in: AtomicU64::new(0),
            bytes_out: AtomicU64::new(0),
            ticks_total: AtomicU64::new(0),
            tick_gap_late: AtomicU64::new(0),
            pkts_out: AtomicU64::new(0),
            total_client_connected_ms: AtomicU64::new(0),
            dispersion_sent: AtomicU64::new(0),
            dispersion_errors: AtomicU64::new(0),
            commands_sent: AtomicU64::new(0),
            commands_errors: AtomicU64::new(0),
            rtt_samples: Mutex::new(Vec::new()),
        }
    }

    /// Appends a round-trip time sample (in milliseconds).
    ///
    /// # Arguments
    ///
    /// * `rtt_ms` - Measured RTT in milliseconds.
    pub fn push_rtt(&self, rtt_ms: u32) {
        if let Ok(mut v) = self.rtt_samples.lock() {
            v.push(rtt_ms);
        }
    }

    /// Prints a periodic one-line progress summary to stdout.
    ///
    /// # Arguments
    ///
    /// * `elapsed` - Wall-clock time since the test started.
    pub fn print_periodic(&self, elapsed: Duration) {
        let connected = self.connected.load(Ordering::Relaxed);
        let errors = self.connect_errors.load(Ordering::Relaxed);
        let ticks = self.ticks_total.load(Ordering::Relaxed);
        let bytes_in = self.bytes_in.load(Ordering::Relaxed);
        let bytes_out = self.bytes_out.load(Ordering::Relaxed);
        let pkts_out = self.pkts_out.load(Ordering::Relaxed);
        let late = self.tick_gap_late.load(Ordering::Relaxed);

        // Use currently-active clients (connected − already-disconnected) so the
        // rate isn't diluted during ramp-up when not all clients have joined yet.
        let disconnects = self.disconnects.load(Ordering::Relaxed);
        let active = connected.saturating_sub(disconnects).max(1);
        let tps = if elapsed.as_secs_f64() > 0.0 {
            ticks as f64 / elapsed.as_secs_f64() / active as f64
        } else {
            0.0
        };

        let dispersion_sent = self.dispersion_sent.load(Ordering::Relaxed);
        let dispersion_errors = self.dispersion_errors.load(Ordering::Relaxed);
        let dispersion_suffix = if dispersion_sent > 0 || dispersion_errors > 0 {
            format!(" dispersion={dispersion_sent}/{dispersion_errors}")
        } else {
            String::new()
        };

        println!(
            "[{:>6.1}s] clients={active}/{connected} errors={errors} | \
             ticks={ticks} (~{tps:.1}/s/client*) late_gaps={late} | \
             in={} out={} pkts_out={pkts_out}{dispersion_suffix}",
            elapsed.as_secs_f64(),
            human_bytes(bytes_in),
            human_bytes(bytes_out),
        );
    }

    /// Prints the final detailed report to stdout.
    ///
    /// # Arguments
    ///
    /// * `elapsed` - Total wall-clock duration of the test.
    pub fn print_final(&self, elapsed: Duration) {
        let connected = self.connected.load(Ordering::Relaxed);
        let errors = self.connect_errors.load(Ordering::Relaxed);
        let disconnects = self.disconnects.load(Ordering::Relaxed);
        let ticks = self.ticks_total.load(Ordering::Relaxed);
        let bytes_in = self.bytes_in.load(Ordering::Relaxed);
        let bytes_out = self.bytes_out.load(Ordering::Relaxed);
        let pkts_out = self.pkts_out.load(Ordering::Relaxed);
        let late = self.tick_gap_late.load(Ordering::Relaxed);

        // Use sum of actual per-client connection durations so bootstrap time
        // and ramp-up delay are excluded from the denominator.
        let total_conn_ms = self.total_client_connected_ms.load(Ordering::Relaxed);
        let (tps, tps_note) = if total_conn_ms > 0 {
            let conn_secs = total_conn_ms as f64 / 1000.0;
            (ticks as f64 / conn_secs, "")
        } else if elapsed.as_secs_f64() > 0.0 && connected > 0 {
            // Fallback when no client has disconnected yet (should not happen in final).
            (
                ticks as f64 / elapsed.as_secs_f64() / connected as f64,
                " (approx; no client has disconnected)",
            )
        } else {
            (0.0, "")
        };

        println!("\n=== Load Test Final Report ===");
        println!("Duration: {:.1}s", elapsed.as_secs_f64());
        println!("--- Connections ---");
        println!("  Connected:     {connected}");
        println!("  Errors:        {errors}");
        println!("  Disconnected:  {disconnects}");
        println!("--- Server Ticks ---");
        let avg_conn_secs = if connected > 0 {
            total_conn_ms as f64 / 1000.0 / connected as f64
        } else {
            0.0
        };
        println!("  Total ticks:   {ticks}");
        println!("  Avg conn time: {avg_conn_secs:.1}s/client");
        println!("  Avg ticks/s/client: {tps:.2} (target: 36.00){tps_note}");
        println!("  Late gaps (>100ms): {late}");
        println!("--- Throughput ---");
        println!("  Bytes in:      {}", human_bytes(bytes_in));
        println!("  Bytes out:     {}", human_bytes(bytes_out));
        println!("  Pkts out:      {pkts_out}");

        let dispersion_sent = self.dispersion_sent.load(Ordering::Relaxed);
        let dispersion_errors = self.dispersion_errors.load(Ordering::Relaxed);
        if dispersion_sent > 0 || dispersion_errors > 0 {
            println!("--- Dispersion ---");
            println!("  Sent:          {dispersion_sent}");
            println!("  Errors:        {dispersion_errors}");
        }

        let commands_sent = self.commands_sent.load(Ordering::Relaxed);
        let commands_errors = self.commands_errors.load(Ordering::Relaxed);
        if commands_sent > 0 || commands_errors > 0 {
            println!("--- Periodic Commands ---");
            println!("  Sent:          {commands_sent}");
            println!("  Errors:        {commands_errors}");
        }

        // RTT stats
        if let Ok(samples) = self.rtt_samples.lock() {
            if samples.is_empty() {
                println!("--- RTT (CL_PING/SV_PONG) ---");
                println!("  No RTT samples collected.");
            } else {
                let mut sorted = samples.clone();
                sorted.sort_unstable();
                let n = sorted.len();
                let min = sorted[0];
                let max = sorted[n - 1];
                let avg = sorted.iter().map(|&v| u64::from(v)).sum::<u64>() / n as u64;
                let p95 = sorted[(n as f64 * 0.95) as usize];
                println!("--- RTT (CL_PING/SV_PONG, n={n}) ---");
                println!("  min={min}ms  avg={avg}ms  p95={p95}ms  max={max}ms");
            }
        }

        println!("==============================\n");
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Formats a byte count as a human-readable string (B / KiB / MiB / GiB).
fn human_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes}B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KiB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1}MiB", bytes as f64 / 1024.0 / 1024.0)
    } else {
        format!("{:.1}GiB", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rtt_percentile_calculation() {
        let m = Metrics::new();
        for v in [10, 20, 30, 40, 50, 60, 70, 80, 90, 100] {
            m.push_rtt(v);
        }
        let samples = m.rtt_samples.lock().unwrap();
        let mut sorted = samples.clone();
        sorted.sort_unstable();
        assert_eq!(sorted[0], 10);
        assert_eq!(sorted[9], 100);
    }

    #[test]
    fn human_bytes_formatting() {
        assert_eq!(human_bytes(512), "512B");
        assert_eq!(human_bytes(1024), "1.0KiB");
        assert_eq!(human_bytes(1536), "1.5KiB");
    }
}
