use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub battery_core_percentage: u32,
    pub ac_core_percentage: u32, // Added for AC mode
    pub transition_delay_ms: u64,
    pub check_interval_sec: u64,
    pub cpu_load_threshold: f32,
    pub ac_cpu_load_threshold: f32, // Added for AC mode
    pub min_cores: usize,
    pub min_change_interval_sec: u64,
    pub load_window_sec: u64,
    pub battery_epp: String, // Add EPP setting
    pub ac_epp: String,      // Add EPP setting
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            battery_core_percentage: 50,
            ac_core_percentage: 100,
            transition_delay_ms: 500,
            check_interval_sec: 5,
            cpu_load_threshold: 45.0,
            ac_cpu_load_threshold: 80.0,
            min_cores: 2,
            min_change_interval_sec: 15, // Reduced default
            load_window_sec: 30,
            battery_epp: "balance_power".to_string(), // Set default
            ac_epp: "balance_performance".to_string(), // Set default
        }
    }
}
