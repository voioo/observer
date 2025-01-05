use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct Settings {
    pub battery_core_percentage: u8,
    pub transition_delay_ms: u64,
    pub check_interval_sec: u64,
    pub cpu_load_threshold: f32,
    pub min_cores: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            battery_core_percentage: 50,
            transition_delay_ms: 500,
            check_interval_sec: 5,
            cpu_load_threshold: 40.0,
            min_cores: 2,
        }
    }
}
