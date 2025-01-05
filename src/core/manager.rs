use log::{debug, error, info};
use std::error::Error;
use std::fs;
use std::thread;
use std::time::Duration;
use sysinfo::System;

use super::topology::CPUTopology;
use crate::config::Settings;

pub struct CoreManager {
    settings: Settings,
    sys: System,
    current_cores: usize,
    topology: CPUTopology,
}

impl CoreManager {
    pub fn new(settings: Settings) -> Self {
        CoreManager {
            settings,
            sys: System::new_all(),
            current_cores: num_cpus::get(),
            topology: CPUTopology::new(),
        }
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
        let total_cores = Self::get_available_cores().len();

        let base_cores = if on_battery {
            (total_cores as f32 * (self.settings.battery_core_percentage as f32 / 100.0)) as usize
        } else {
            total_cores
        };

        // Calculate average load
        let active_cpus: Vec<_> = self
            .sys
            .cpus()
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                if *i == 0 {
                    return true;
                }
                let cpu_path = format!("/sys/devices/system/cpu/cpu{}/online", i);
                fs::read_to_string(&cpu_path)
                    .map(|content| content.trim() == "1")
                    .unwrap_or(false)
            })
            .collect();

        let avg_load = active_cpus
            .iter()
            .map(|(_, cpu)| cpu.cpu_usage())
            .sum::<f32>()
            / active_cpus.len() as f32;

        debug!("Current CPU load: {:.2}%", avg_load);

        // Adjust cores based on load
        let mut optimal_cores = if avg_load > self.settings.cpu_load_threshold {
            total_cores
        } else if avg_load < self.settings.cpu_load_threshold / 2.0 {
            base_cores.saturating_sub(2)
        } else {
            base_cores
        };

        optimal_cores = optimal_cores.max(self.settings.min_cores).min(total_cores);
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
