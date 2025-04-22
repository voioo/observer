use crate::config::Settings;
use crate::system::PowerState;
use log::{debug, error, info, warn};
use std::error::Error;
#[cfg(target_os = "linux")]
use std::fs;
use std::path::Path;
#[cfg(target_os = "linux")]
use std::thread;
use std::time::Duration;
use sysinfo::System;

use super::load_tracker::LoadTracker;
#[cfg(target_os = "linux")]
use super::topology::CPUTopology;

pub struct CoreManager {
    settings: Settings,
    #[allow(dead_code)] // Temporary until fully implemented
    topology: CPUTopology,
    sys: System,
    current_cores: usize,
    load_tracker: LoadTracker,
    last_power_state: Option<PowerState>,
}

impl CoreManager {
    pub fn new(settings: crate::config::Settings) -> Result<Self, Box<dyn Error>> {
        let settings_clone = settings.clone();
        #[cfg(target_os = "linux")]
        let topology = CPUTopology::new();
        #[cfg(not(target_os = "linux"))]
        let topology = CPUTopology::default();

        let total_cores = topology.num_p_cores + topology.num_e_cores;
        let initial_cores = topology.cores.len() * 2;
        info!(
            "Initializing CoreManager. Found {} physical cores, {} logical cores initially online.",
            total_cores, initial_cores
        );
        Ok(Self {
            settings: settings_clone.clone(),
            topology,
            sys: System::new_all(),
            current_cores: initial_cores,
            load_tracker: LoadTracker::new(Duration::from_secs(
                settings_clone.load_window_sec,
            )),
            last_power_state: None,
        })
    }

    #[cfg(target_os = "linux")]
    fn calculate_current_load(&self) -> f32 {
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
    pub fn get_available_cores() -> Result<Vec<usize>, Box<dyn Error>> {
        let mut cores = Vec::new();
        let cpu_path = Path::new("/sys/devices/system/cpu");

        for i in 0..256 {
            let core_path = cpu_path.join(format!("cpu{}", i));
            if core_path.exists() {
                if i == 0 || core_path.join("online").exists() {
                    cores.push(i);
                } else {
                    debug!(
                        "Core {} directory exists but 'online' file missing, not adding.",
                        i
                    );
                }
            } else if i > 0 {
                // Don't break immediately if cpu0 is missing for some reason
                break;
            }
        }

        if cores.is_empty() {
            Err("No CPU cores found in /sys/devices/system/cpu".into())
        } else {
            Ok(cores)
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn get_available_cores() -> Result<Vec<usize>, Box<dyn Error>> {
        warn!("Core enumeration through /sysfs is only supported on Linux. Reporting core 0 only.");
        Ok(vec![0]) // Return core 0 as a default/fallback
    }

    pub fn get_optimal_core_count(&mut self, on_battery: bool) -> Result<usize, Box<dyn Error>> {
        self.sys.refresh_cpu_all();

        let current_load = self.calculate_current_load();
        self.load_tracker.add_measurement(current_load);

        let time_since_last_change = self.load_tracker.time_since_last_change();

        if time_since_last_change < Duration::from_secs(self.settings.min_change_interval_sec) {
            debug!("Skipping core adjustment - min interval not reached");
            return Ok(self.current_cores);
        }

        let avg_load = self.load_tracker.get_average();
        let total_cores = self.sys.cpus().len();
        let min_cores = self.settings.min_cores;

        let load_threshold = if on_battery {
            self.settings.cpu_load_threshold
        } else {
            self.settings.ac_cpu_load_threshold
        };

        let core_percentage = if on_battery {
            self.settings.battery_core_percentage
        } else {
            self.settings.ac_core_percentage
        };
        let percentage_limit = (total_cores as f32 * (core_percentage as f32 / 100.0))
            .ceil()
            .max(min_cores as f32) as usize;

        let target_cores = if avg_load > load_threshold * 1.2 && self.current_cores < total_cores {
            (self.current_cores + 2).min(total_cores)
        } else if avg_load < load_threshold * 0.8 && self.current_cores > min_cores {
            (self.current_cores.saturating_sub(2))
                .max(min_cores)
                .min(percentage_limit)
        } else {
            self.current_cores
        };

        let optimal_cores = target_cores;

        if optimal_cores != self.current_cores {
            self.load_tracker.record_change();
            info!(
                "Targeting {} cores (current: {}, limit: {}, load: {:.1}%, on_battery: {})",
                optimal_cores, self.current_cores, percentage_limit, avg_load, on_battery
            );
        }

        let current_power_state = if on_battery {
            PowerState::Battery
        } else {
            PowerState::AC
        };
        if self.last_power_state != Some(current_power_state) {
            let epp_hint = match current_power_state {
                PowerState::AC => &self.settings.ac_epp,
                PowerState::Battery => &self.settings.battery_epp,
                PowerState::Unknown => "balance_performance",
            };
            info!(
                "Power state changed to {:?}. Setting EPP hint to '{}'",
                current_power_state, epp_hint
            );
            if let Err(e) = set_epp_hint(epp_hint) {
                error!("Failed to set EPP hint: {}", e);
            }
            self.last_power_state = Some(current_power_state);
        }

        Ok(optimal_cores)
    }

    #[cfg(target_os = "linux")]
    fn perform_core_state_changes(&mut self, target_cores: usize) -> Result<(), Box<dyn Error>> {
        let available_cores = Self::get_available_cores()?;
        let mut operation_successful = true;
        let mut last_error: Option<Box<dyn Error>> = None;

        for core_num in available_cores.iter().skip(1) {
            let should_enable = core_num < &target_cores;
            let cpu_state_path = format!("/sys/devices/system/cpu/cpu{}/online", core_num);

            let current_state_result = fs::read_to_string(&cpu_state_path);
            let currently_enabled = match current_state_result {
                Ok(content) => content.trim() == "1",
                Err(e) => {
                    error!(
                        "Linux: Failed to read current state for core {}: {}. Skipping change.",
                        core_num, e
                    );
                    operation_successful = false;
                    last_error = Some(e.into());
                    continue;
                }
            };

            if should_enable == currently_enabled {
                continue;
            }

            debug!(
                "Linux: Attempting to {} core {}",
                if should_enable { "enable" } else { "disable" },
                core_num
            );
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
    fn perform_core_state_changes(&self, target_cores: usize) -> Result<(), Box<dyn Error>> {
        warn!(
            "Core enable/disable is only supported on Linux. Requested {} cores.",
            target_cores
        );
        Ok(())
    }

    pub fn manage_cpu_cores(&mut self, target_cores: usize) -> Result<(), Box<dyn Error>> {
        if target_cores == self.current_cores {
            return Ok(());
        }

        match self.perform_core_state_changes(target_cores) {
            Ok(_) => {
                info!("Successfully adjusted cores to target: {}", target_cores);
                self.current_cores = target_cores;
                Ok(())
            }
            Err(e) => {
                error!("Errors occurred while adjusting cores. Target {} may not have been fully reached. Error: {}", target_cores, e);
                Err(e)
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn enable_all_cores(&self) {
        info!("Linux: Cleaning up - restoring all cores...");
        let available_cores = Self::get_available_cores().unwrap();
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
        info!("Linux: Restoring default EPP hint ('balance_performance')...");
        if let Err(e) = set_epp_hint("balance_performance") {
            error!("Failed to restore default EPP hint during cleanup: {}", e);
        }
        info!("Linux: Cleanup complete - all cores should be enabled");
    }

    #[cfg(not(target_os = "linux"))]
    fn enable_all_cores(&self) {
        info!("Non-Linux: Cleanup complete (no core state changes performed).");
    }
}

impl Drop for CoreManager {
    fn drop(&mut self) {
        self.enable_all_cores();
    }
}

#[cfg(target_os = "linux")]
fn set_epp_hint(hint: &str) -> Result<(), String> {
    debug!("Attempting to set EPP hint to '{}' for all policies", hint);
    let base_path = Path::new("/sys/devices/system/cpu/cpufreq");
    let mut policies_updated = 0;

    for entry in fs::read_dir(base_path)
        .map_err(|e| format!("Failed to read {}: {}", base_path.display(), e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name() {
                if name.to_string_lossy().starts_with("policy") {
                    let epp_path = path.join("energy_performance_preference");
                    if epp_path.exists() {
                        match fs::write(&epp_path, hint) {
                            Ok(_) => {
                                debug!(
                                    "Successfully set EPP for {} to '{}'",
                                    name.to_string_lossy(),
                                    hint
                                );
                                policies_updated += 1;
                            }
                            Err(e) => {
                                if e.kind() == std::io::ErrorKind::PermissionDenied {
                                    error!(
                                        "Permission denied writing to {}. Run observer with sudo?",
                                        epp_path.display()
                                    );
                                    return Err(format!(
                                        "Permission denied for {}",
                                        epp_path.display()
                                    ));
                                } else {
                                    warn!("Failed to write to {}: {}. Check permissions or if file is writable.", epp_path.display(), e);
                                }
                            }
                        }
                    } else {
                        debug!(
                            "EPP file not found for {}: {}",
                            name.to_string_lossy(),
                            epp_path.display()
                        );
                    }
                }
            }
        }
    }

    if policies_updated == 0 {
        warn!(
            "Could not set EPP hint for any CPU policy. Is intel_pstate active and EPP available?"
        );
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn set_epp_hint(hint: &str) -> Result<(), String> {
    warn!(
        "EPP setting is only supported on Linux. Hint '{}' ignored.",
        hint
    );
    Ok(())
}
