use log::{debug, error, info, warn};
use std::error::Error;
#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::thread;
use std::time::Duration;
use sysinfo::System;

use super::load_tracker::LoadTracker;
#[cfg(target_os = "linux")]
use super::topology::CPUTopology; // Guarded import
use crate::config::Settings;

#[cfg(not(target_os = "linux"))]
use num_cpus; // Only needed for non-Linux get_available_cores

pub struct CoreManager {
    settings: Settings,
    sys: System,
    current_cores: usize,
    #[cfg(target_os = "linux")]
    topology: CPUTopology, // Only include topology field on Linux
    load_tracker: LoadTracker,
}

impl CoreManager {
    pub fn new(settings: Settings) -> Self {
        let initial_cores = Self::get_available_cores().len();
        CoreManager {
            settings: settings.clone(),
            sys: System::new_all(),
            current_cores: initial_cores,
            #[cfg(target_os = "linux")]
            topology: CPUTopology::new(), // Initialize only on Linux
            load_tracker: LoadTracker::new(Duration::from_secs(settings.load_window_sec)),
        }
    }

    pub fn current_cores(&self) -> usize {
        self.current_cores
    }

    #[cfg(target_os = "linux")]
    fn calculate_current_load(&self) -> f32 {
        // Linux: Check /sysfs to find truly active cores
        let active_cpus: Vec<_> = self
            .sys
            .cpus()
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                if *i == 0 {
                    return true; // CPU0 always active
                }
                let cpu_path = format!("/sys/devices/system/cpu/cpu{}/online", i);
                match fs::read_to_string(&cpu_path) {
                    Ok(content) => content.trim() == "1",
                    Err(_) => false, // Assume offline if cannot read state
                }
            })
            .collect();

        let active_count = active_cpus.len().max(1); // Avoid division by zero

        let total_load: f32 = active_cpus
            .iter()
            .map(|(_, cpu)| cpu.cpu_usage())
            .sum::<f32>();

        let avg_load = total_load / active_count as f32;
        debug!(
            "Linux Load calc: total={:.2}% across {} active cores, avg={:.2}%",
            total_load, active_count, avg_load
        );
        total_load // Return total load as before
    }

    #[cfg(not(target_os = "linux"))]
    fn calculate_current_load(&self) -> f32 {
        // Non-Linux: Assume all logical cores reported by sysinfo are active
        let cpus = self.sys.cpus();
        let count = cpus.len().max(1);
        let total_load: f32 = cpus.iter().map(|cpu| cpu.cpu_usage()).sum();
        let avg_load = total_load / count as f32;
        debug!(
            "Non-Linux Load calc: total={:.2}% across {} logical cores, avg={:.2}%",
            total_load, count, avg_load
        );
        total_load // Return total load
    }

    #[cfg(target_os = "linux")]
    pub fn get_available_cores() -> Vec<usize> {
        // Linux: Read /sysfs to find potentially available cores
        let mut available_cores = Vec::new();
        let cpu_path = "/sys/devices/system/cpu";
        debug!("Checking for available cores in {}", cpu_path);

        if let Ok(entries) = fs::read_dir(cpu_path) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("cpu") {
                    if let Ok(num) = name_str.trim_start_matches("cpu").parse::<usize>() {
                        // Check if core 0 or if the 'online' file exists (indicates a controllable core)
                        let online_path = format!("{}/cpu{}/online", cpu_path, num);
                        if num == 0 || fs::metadata(&online_path).is_ok() {
                            available_cores.push(num);
                        }
                    }
                }
            }
        }
        available_cores.sort();
        debug!("Linux - Found available cores: {:?}", available_cores);
        if available_cores.is_empty() {
            warn!("Could not find any cores in /sysfs! Defaulting to core 0.");
            vec![0] // Fallback
        } else {
            available_cores
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn get_available_cores() -> Vec<usize> {
        // Non-Linux: Use num_cpus crate
        let count = num_cpus::get().max(1);
        let cores: Vec<usize> = (0..count).collect();
        debug!("Non-Linux - Found available cores: {:?}", cores);
        cores
    }

    pub fn get_optimal_core_count(&mut self, on_battery: bool) -> usize {
        self.sys.refresh_cpu_all();

        let current_load = self.calculate_current_load();
        // debug!("Current CPU load: {:.2}%", current_load); // Already logged in calculate_current_load
        self.load_tracker.add_measurement(current_load);

        let time_since_last_change = self.load_tracker.time_since_last_change();
        // debug!( // Reduced verbosity
        //     "Time since last core change: {:.2}s (minimum: {}s)",
        //     time_since_last_change.as_secs_f64(),
        //     self.settings.min_change_interval_sec
        // );

        if time_since_last_change < Duration::from_secs(self.settings.min_change_interval_sec) {
            debug!("Skipping core adjustment - min interval not reached");
            return self.current_cores;
        }

        let avg_load = self.load_tracker.get_average();
        let total_cores = Self::get_available_cores().len();

        let load_threshold = if on_battery {
            self.settings.cpu_load_threshold
        } else {
            self.settings.ac_cpu_load_threshold
        };

        // Hysteresis-based decision
        let target_cores = if avg_load > load_threshold * 1.2 && self.current_cores < total_cores {
            // Increase if load is high AND we aren't already at max cores
            debug!(
                "Load ({:.2}%) > threshold*1.2 ({:.2}%), increasing cores",
                 avg_load, load_threshold * 1.2
            );
            (self.current_cores + 2).min(total_cores) // Increase by 2, but don't exceed total
        } else if avg_load < load_threshold * 0.5 && self.current_cores > self.settings.min_cores {
            // Decrease if load is low AND we aren't already at min cores
            debug!(
                "Load ({:.2}%) < threshold*0.5 ({:.2}%), decreasing cores",
                avg_load, load_threshold * 0.5
            );
            (self.current_cores.saturating_sub(2)).max(self.settings.min_cores) // Decrease by 2, but don't go below min
        } else {
            // Maintain current cores if load is stable or we are already at limits
            // debug!( // Reduced verbosity
            //     "Load within threshold range ({:.2}%) or at limits, maintaining cores (on_battery: {})",
            //     avg_load, on_battery
            // );
            self.current_cores
        };

        // Apply core percentage limit if applicable (primarily for battery, but respects AC setting too)
        let core_percentage = if on_battery {
            self.settings.battery_core_percentage
        } else {
            self.settings.ac_core_percentage
        };
        let percentage_limit = (total_cores as f32 * (core_percentage as f32 / 100.0)).ceil().max(1.0) as usize;

        // Final core count is the hysteresis target, clamped by the percentage limit and min_cores
        let optimal_cores = target_cores.clamp(self.settings.min_cores, percentage_limit);

        if optimal_cores != self.current_cores {
            self.load_tracker.record_change();
            info!(
                "Targeting {} cores (current: {}, limit: {}, load: {:.1}%, on_battery: {})",
                optimal_cores, self.current_cores, percentage_limit, avg_load, on_battery
            );
        }

        optimal_cores
    }

    #[cfg(target_os = "linux")]
    fn perform_core_state_changes(&mut self, target_cores: usize) -> Result<(), Box<dyn Error>> {
        // Linux: Actual implementation using /sysfs
        let cores_to_enable = self.topology.get_cores_to_enable(target_cores);
        info!("Linux: Attempting to set {} active cores. Enabling cores: {:?}", target_cores, cores_to_enable);

        let available_cores = Self::get_available_cores();
        let mut operation_successful = true;
        let mut last_error: Option<Box<dyn Error>> = None;

        for core_num in available_cores.iter().skip(1) { // Always skip core 0
            let should_enable = cores_to_enable.contains(core_num);
            let cpu_state_path = format!("/sys/devices/system/cpu/cpu{}/online", core_num);

            let current_state_result = fs::read_to_string(&cpu_state_path);
            let currently_enabled = match current_state_result {
                Ok(content) => content.trim() == "1",
                Err(e) => {
                    error!("Linux: Failed to read current state for core {}: {}. Skipping change.", core_num, e);
                    operation_successful = false;
                    last_error = Some(e.into());
                    continue;
                }
            };

            if should_enable == currently_enabled {
                // debug!("Linux: Core {} already in desired state ({}).", core_num, if should_enable {"enabled"} else {"disabled"});
                continue;
            }

            debug!("Linux: Attempting to {} core {}", if should_enable {"enable"} else {"disable"}, core_num);
            if let Err(e) = fs::write(&cpu_state_path, if should_enable { "1" } else { "0" }) {
                error!(
                    "Linux: Failed to {} core {}: {}",
                    if should_enable { "enable" } else { "disable" },
                    core_num,
                    e
                );
                operation_successful = false;
                last_error = Some(e.into());
            } else {
                debug!(
                    "Linux: Core {} successfully {}",
                    core_num,
                    if should_enable { "enabled" } else { "disabled" }
                );
                if should_enable {
                    thread::sleep(Duration::from_millis(self.settings.transition_delay_ms));
                }
            }
        }

        if operation_successful {
            Ok(())
        } else {
            Err(last_error.unwrap_or_else(|| "Unknown error during Linux core management".into()))
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn perform_core_state_changes(&self, target_cores: usize) -> Result<(), Box<dyn Error>> { // Note: Now uses &self, no longer &mut
        // Non-Linux: Log warning and do nothing physically
        warn!("Core enable/disable is only supported on Linux. Requested {} cores.", target_cores);
        Ok(())
    }

    pub fn manage_cpu_cores(&mut self, target_cores: usize) -> Result<(), Box<dyn Error>> {
        if target_cores == self.current_cores {
            // debug!("Target cores ({}) matches current ({}), no change needed.", target_cores, self.current_cores);
            return Ok(());
        }

        match self.perform_core_state_changes(target_cores) {
            Ok(_) => {
                info!("Successfully adjusted cores to target: {}", target_cores);
                self.current_cores = target_cores; // Update internal state only on success
                Ok(())
            }
            Err(e) => {
                error!("Errors occurred while adjusting cores. Target {} may not have been fully reached. Error: {}", target_cores, e);
                // Don't update self.current_cores if the operation failed
                Err(e)
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn enable_all_cores(&self) {
        // Linux: Attempt to enable all found cores on drop
        info!("Linux: Cleaning up - restoring all cores...");
        let available_cores = Self::get_available_cores();
        for core_num in available_cores.iter().skip(1) {
            let cpu_state_path = format!("/sys/devices/system/cpu/cpu{}/online", core_num);
            match fs::write(&cpu_state_path, "1") {
                Ok(_) => debug!("Linux: Enabled core {} on shutdown.", core_num),
                Err(e) => warn!(
                    "Linux: Failed to enable core {} on shutdown: {}",
                    core_num, e
                ),
            }
        }
        info!("Linux: Cleanup complete - all cores should be enabled");
    }

    #[cfg(not(target_os = "linux"))]
    fn enable_all_cores(&self) {
        // Non-Linux: Do nothing
        info!("Non-Linux: Cleanup complete (no core state changes performed).");
    }
}

impl Drop for CoreManager {
    fn drop(&mut self) {
        self.enable_all_cores();
    }
}
