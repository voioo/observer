use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub battery_core_percentage: u8,
    pub transition_delay_ms: u64,
    pub check_interval_sec: u64,
    pub cpu_load_threshold: f32,
    pub min_cores: usize,
    pub min_change_interval_sec: u64,
    pub load_window_sec: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            battery_core_percentage: 50,
            transition_delay_ms: 500,
            check_interval_sec: 5,
            cpu_load_threshold: 75.0,
            min_cores: 2,
            min_change_interval_sec: 30,
            load_window_sec: 30,
        }
    }
}
