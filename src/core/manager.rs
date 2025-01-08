use log::{debug, error, info};
use std::error::Error;
use std::fs;
use std::thread;
use std::time::Duration;
use sysinfo::System;

use super::load_tracker::LoadTracker;
use super::topology::CPUTopology;
use crate::config::Settings;

pub struct CoreManager {
    settings: Settings,
    sys: System,
    current_cores: usize,
    topology: CPUTopology,
    load_tracker: LoadTracker,
}

impl CoreManager {
    pub fn new(settings: Settings) -> Self {
        CoreManager {
            settings: settings.clone(),
            sys: System::new_all(),
            current_cores: num_cpus::get(),
            topology: CPUTopology::new(),
            load_tracker: LoadTracker::new(Duration::from_secs(settings.load_window_sec)),
        }
    }

    pub fn current_cores(&self) -> usize {
        self.current_cores
    }

    fn calculate_current_load(&self) -> f32 {
        let active_cpus: Vec<_> = self
            .sys
            .cpus()
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                if *i == 0 {
                    return true;
                } // CPU0 always active
                let cpu_path = format!("/sys/devices/system/cpu/cpu{}/online", i);
                fs::read_to_string(&cpu_path)
                    .map(|content| content.trim() == "1")
                    .unwrap_or(false)
            })
            .collect();

        let active_count = active_cpus.len().max(1); // Avoid division by zero

        let total_load: f32 = active_cpus
            .iter()
            .map(|(_, cpu)| cpu.cpu_usage())
            .sum::<f32>();

        let avg_load = total_load / active_count as f32;
        debug!(
            "Load calculation: total_load={:.2}% across {} active cores, avg_load={:.2}%",
            total_load, active_count, avg_load
        );

        total_load
    }

    pub fn get_available_cores() -> Vec<usize> {
        let mut available_cores = Vec::new();
        let cpu_path = "/sys/devices/system/cpu";

        if let Ok(entries) = fs::read_dir(cpu_path) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("cpu") {
                    if let Ok(num) = name_str.trim_start_matches("cpu").parse::<usize>() {
                        if num == 0
                            || fs::metadata(format!("{}/cpu{}/online", cpu_path, num)).is_ok()
                        {
                            available_cores.push(num);
                        }
                    }
                }
            }
        }

        available_cores.sort();
        debug!("Found available cores: {:?}", available_cores);
        available_cores
    }

    pub fn get_optimal_core_count(&mut self, on_battery: bool) -> usize {
        self.sys.refresh_cpu_all();

        let current_load = self.calculate_current_load();
        debug!("Current CPU load: {:.2}%", current_load);
        self.load_tracker.add_measurement(current_load);

        let time_since_last_change = self.load_tracker.time_since_last_change();
        debug!(
            "Time since last core change: {:.2}s (minimum: {}s)",
            time_since_last_change.as_secs_f64(),
            self.settings.min_change_interval_sec
        );

        if time_since_last_change < Duration::from_secs(self.settings.min_change_interval_sec) {
            debug!(
                "Skipping core adjustment - min interval not reached ({:.2}s remaining)",
                self.settings.min_change_interval_sec as f64 - time_since_last_change.as_secs_f64()
            );
            return self.current_cores;
        }

        let avg_load = self.load_tracker.get_average();
        let total_cores = Self::get_available_cores().len();

        let base_cores = if on_battery {
            (total_cores as f32 * (self.settings.battery_core_percentage as f32 / 100.0)) as usize
        } else {
            total_cores
        };

        // Hysteresis-based decision
        let target_cores = if on_battery {
            if avg_load > self.settings.cpu_load_threshold * 1.2 {
                // 20% above threshold
                debug!(
                    "Load significantly high ({:.2}%), increasing cores",
                    avg_load
                );
                (base_cores + 2).min(total_cores)
            } else if avg_load < self.settings.cpu_load_threshold * 0.5 {
                // 50% below threshold
                debug!(
                    "Load significantly low ({:.2}%), decreasing cores",
                    avg_load
                );
                base_cores.saturating_sub(2)
            } else {
                debug!(
                    "Load within threshold range ({:.2}%), maintaining cores",
                    avg_load
                );
                self.current_cores
            }
        } else {
            total_cores
        };

        let optimal_cores = target_cores.clamp(self.settings.min_cores, total_cores);

        if optimal_cores != self.current_cores {
            self.load_tracker.record_change();
            info!(
                "Adjusting cores from {} to {} (load: {:.2}%, on_battery: {})",
                self.current_cores,
                optimal_cores,
                avg_load,
                on_battery
            );
        }

        optimal_cores
    }

    pub fn manage_cpu_cores(&mut self, target_cores: usize) -> Result<(), Box<dyn Error>> {
        if target_cores == self.current_cores {
            return Ok(());
        }

        let cores_to_enable = self.topology.get_cores_to_enable(target_cores);
        info!("Planning to enable cores: {:?}", cores_to_enable);

        let available_cores = Self::get_available_cores();

        for core_num in available_cores.iter().skip(1) {
            let should_enable = cores_to_enable.contains(core_num);
            let cpu_state_path = format!("/sys/devices/system/cpu/cpu{}/online", core_num);

            if let Err(e) = fs::write(&cpu_state_path, if should_enable { "1" } else { "0" }) {
                error!(
                    "Failed to {} core {}: {}",
                    if should_enable { "enable" } else { "disable" },
                    core_num,
                    e
                );
            } else {
                debug!(
                    "Core {} {}",
                    core_num,
                    if should_enable { "enabled" } else { "disabled" }
                );
            }

            if should_enable {
                thread::sleep(Duration::from_millis(self.settings.transition_delay_ms));
            }
        }

        self.current_cores = target_cores;
        info!("Core count adjusted to: {}", self.current_cores);
        Ok(())
    }
}

impl Drop for CoreManager {
    fn drop(&mut self) {
        info!("Cleaning up - restoring all cores...");

        let available_cores = Self::get_available_cores();
        for core_num in available_cores.iter().skip(1) {
            let online_path = format!("/sys/devices/system/cpu/cpu{}/online", core_num);
            match fs::write(&online_path, "1") {
                Ok(_) => debug!("Enabled core {} during cleanup", core_num),
                Err(e) => error!("Failed to enable core {} during cleanup: {}", core_num, e),
            }
        }

        info!("Cleanup complete - all cores should be enabled");
    }
}
