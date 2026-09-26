use std::time::Instant;

pub struct ProgressTracker {
    total_bytes: u64,
    transferred_bytes: u64,
    start_time: Instant,
    last_update_time: Instant,
    last_transferred_bytes: u64,
    current_speed_bps: u64,
}

impl ProgressTracker {
    pub fn new(total_bytes: u64, initial_transferred: u64) -> Self {
        let now = Instant::now();
        Self {
            total_bytes,
            transferred_bytes: initial_transferred,
            start_time: now,
            last_update_time: now,
            last_transferred_bytes: initial_transferred,
            current_speed_bps: 0,
        }
    }

    pub fn update(&mut self, current_transferred: u64) {
        self.transferred_bytes = current_transferred;
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update_time).as_secs_f64();

        if elapsed >= 0.5 {
            let bytes_delta = self.transferred_bytes.saturating_sub(self.last_transferred_bytes);
            self.current_speed_bps = (bytes_delta as f64 / elapsed) as u64;
            self.last_update_time = now;
            self.last_transferred_bytes = self.transferred_bytes;
        }
    }

    pub fn speed_bytes_per_sec(&self) -> u64 {
        self.current_speed_bps
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    pub fn eta_seconds(&self) -> Option<u64> {
        if self.current_speed_bps == 0 {
            return None;
        }
        let remaining = self.total_bytes.saturating_sub(self.transferred_bytes);
        Some(remaining / self.current_speed_bps)
    }

    pub fn percentage(&self) -> f64 {
        if self.total_bytes == 0 {
            100.0
        } else {
            (self.transferred_bytes as f64 / self.total_bytes as f64) * 100.0
        }
    }
}
