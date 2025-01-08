use log::debug;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub struct LoadTracker {
    history: VecDeque<(f32, Instant)>,
    window_size: Duration,
    pub last_change: Instant,
}

impl LoadTracker {
    pub fn new(window_size: Duration) -> Self {
        Self {
            history: VecDeque::new(),
            window_size,
            last_change: Instant::now(),
        }
    }

    pub fn add_measurement(&mut self, load: f32) {
        let now = Instant::now();

        self.history.push_back((load, now));
        debug!("Added load measurement: {:.2}%", load);

        let cutoff = now - self.window_size;
        let old_len = self.history.len();

        while let Some((_, time)) = self.history.front() {
            if *time < cutoff {
                self.history.pop_front();
            } else {
                break;
            }
        }

        if old_len != self.history.len() {
            debug!(
                "Pruned {} old measurements from history",
                old_len - self.history.len()
            );
        }

        debug!(
            "Current history size: {}, Average load: {:.2}%",
            self.history.len(),
            self.get_average()
        );
    }

    pub fn get_average(&self) -> f32 {
        if self.history.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.history.iter().map(|(load, _)| load).sum();
        sum / self.history.len() as f32
    }

    pub fn record_change(&mut self) {
        let previous = self.last_change;
        self.last_change = Instant::now();
        debug!(
            "Recording core change. Time since previous: {:.2}s",
            self.last_change.duration_since(previous).as_secs_f64()
        );
    }

    pub fn time_since_last_change(&self) -> Duration {
        self.last_change.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_tracker_average() {
        let mut tracker = LoadTracker::new(Duration::from_secs(30));
        tracker.add_measurement(50.0);
        tracker.add_measurement(100.0);
        assert_eq!(tracker.get_average(), 75.0);
    }

    #[test]
    fn test_window_pruning() {
        let window = Duration::from_secs(2);
        let mut tracker = LoadTracker::new(window);

        tracker.add_measurement(50.0);

        std::thread::sleep(Duration::from_secs(3));

        tracker.add_measurement(100.0);

        assert_eq!(tracker.history.len(), 1);
        assert_eq!(tracker.get_average(), 100.0);
    }
}
