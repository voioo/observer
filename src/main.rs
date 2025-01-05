use config::{Config, ConfigError, File};
use log::{debug, error, info, warn};
use serde::Deserialize;
use std::error::Error;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use sysinfo::System;

#[derive(Debug, Deserialize)]
struct Settings {
    battery_core_percentage: u8,
    transition_delay_ms: u64,
    check_interval_sec: u64,
    cpu_load_threshold: f32,
    min_cores: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            battery_core_percentage: 50,
            transition_delay_ms: 500,
            check_interval_sec: 5,
            cpu_load_threshold: 80.0,
            min_cores: 2,
        }
    }
}

struct CPUTopology {
    physical_cores: Vec<(usize, usize)>, // (core_id, ht_sibling_id)
}

impl CPUTopology {
    fn new() -> Self {
        let mut pairs = Vec::new();
        let cpu_path = "/sys/devices/system/cpu";

        for core_id in 0.. {
            let topology_path =
                format!("{}/cpu{}/topology/thread_siblings_list", cpu_path, core_id);

            // Stop if we can't find this CPU
            if fs::metadata(&topology_path).is_err() {
                break;
            }

            // Read thread siblings
            if let Ok(siblings) = fs::read_to_string(&topology_path) {
                let nums: Vec<usize> = siblings
                    .trim()
                    .split(',')
                    .filter_map(|s| s.parse().ok())
                    .collect();

                // Only add each pair once, using the lower number as primary
                if nums.len() == 2 && nums[0] == core_id {
                    pairs.push((nums[0], nums[1]));
                }
            }
        }

        debug!("Found CPU pairs (physical, HT): {:?}", pairs);
        CPUTopology {
            physical_cores: pairs,
        }
    }

    fn get_cores_to_enable(&self, target_count: usize) -> Vec<usize> {
        let mut cores = Vec::new();
        let mut count;

        // Always enable CPU 0 and its sibling if exists
        cores.push(0);
        if let Some((_, sibling)) = self.physical_cores.iter().find(|(core, _)| *core == 0) {
            cores.push(*sibling);
            count = 2;
        } else {
            count = 1;
        }

        // Enable additional cores in pairs
        for &(core, sibling) in self.physical_cores.iter().skip(1) {
            if count >= target_count {
                break;
            }
            cores.push(core);
            cores.push(sibling);
            count += 2;
        }

        cores.sort();
        cores
    }
}

struct CoreManager {
    settings: Settings,
    sys: System,
    current_cores: usize,
    topology: CPUTopology,
}

impl CoreManager {
    fn new(settings: Settings) -> Self {
        CoreManager {
            settings,
            sys: System::new_all(),
            current_cores: num_cpus::get(),
            topology: CPUTopology::new(),
        }
    }

    fn get_available_cores() -> Vec<usize> {
        let mut available_cores = Vec::new();
        let cpu_path = "/sys/devices/system/cpu";

        // First, try to read the topology information
        if let Ok(entries) = fs::read_dir(cpu_path) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("cpu") {
                    if let Ok(num) = name_str.trim_start_matches("cpu").parse::<usize>() {
                        // Always include CPU0
                        if num == 0 {
                            available_cores.push(num);
                            continue;
                        }

                        // Check for both online capability and current state
                        let online_path = format!("{}/cpu{}/online", cpu_path, num);
                        let topology_path =
                            format!("{}/cpu{}/topology/thread_siblings_list", cpu_path, num);

                        // If we can control this core, add it
                        if fs::metadata(&online_path).is_ok() {
                            available_cores.push(num);

                            // Log topology information if available
                            if let Ok(siblings) = fs::read_to_string(&topology_path) {
                                debug!("CPU {} thread siblings: {}", num, siblings.trim());
                            }
                        }
                    }
                }
            }
        }

        available_cores.sort();
        debug!("Found available cores: {:?}", available_cores);
        available_cores
    }

    fn get_optimal_core_count(&mut self, on_battery: bool) -> usize {
        self.sys.refresh_cpu_all();
        let available_cores = Self::get_available_cores();
        let total_cores = available_cores.len();

        // Calculate base number of cores based on power state
        let base_cores = if on_battery {
            (total_cores as f32 * (self.settings.battery_core_percentage as f32 / 100.0)) as usize
        } else {
            total_cores
        };

        // Get average CPU load across active cores only
        let active_cpus: Vec<_> = self
            .sys
            .cpus()
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                if *i == 0 {
                    return true;
                } // CPU0 is always considered active
                let cpu_path = format!("/sys/devices/system/cpu/cpu{}/online", i);
                fs::read_to_string(&cpu_path)
                    .map(|content| content.trim() == "1")
                    .unwrap_or(false)
            })
            .collect();

        let active_count = active_cpus.len().max(1); // Avoid division by zero
        let avg_load: f32 = active_cpus
            .iter()
            .map(|(_, cpu)| cpu.cpu_usage())
            .sum::<f32>()
            / active_count as f32;

        debug!(
            "Current average CPU load across {} active cores: {:.2}%",
            active_count, avg_load
        );

        // Adjust cores based on load
        let mut optimal_cores = if avg_load > self.settings.cpu_load_threshold {
            // More aggressive scaling when load is high
            base_cores + (base_cores / 4).max(1)
        } else if avg_load < self.settings.cpu_load_threshold / 2.0 {
            // Gradual reduction when load is low
            base_cores.saturating_sub(1)
        } else {
            base_cores
        };

        // Ensure we don't go below minimum cores
        optimal_cores = optimal_cores.max(self.settings.min_cores);
        // Ensure we don't exceed total cores
        optimal_cores = optimal_cores.min(total_cores);

        optimal_cores
    }

    fn manage_cpu_cores(&mut self, target_cores: usize) -> Result<(), Box<dyn Error>> {
        if target_cores == self.current_cores {
            return Ok(());
        }

        let cores_to_enable = self.topology.get_cores_to_enable(target_cores);
        info!("Planning to enable cores: {:?}", cores_to_enable);

        // Get all available cores
        let available_cores = Self::get_available_cores();

        // Disable all cores not in our target list
        for core_num in available_cores.iter().skip(1) {
            // Skip CPU0
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
        info!(
            "Core count adjusted to: {} (Enabled cores: {:?})",
            self.current_cores, cores_to_enable
        );
        Ok(())
    }
}

fn load_config() -> Result<Settings, ConfigError> {
    debug!("Attempting to load configuration...");

    let config_paths = [
        "/etc/observer/config.toml",
        "/etc/observer/config",
        "config.toml",
        "config",
    ];

    let mut builder = Config::builder();

    for path in &config_paths {
        debug!("Checking for config at: {}", path);
        builder = builder.add_source(File::with_name(path).required(false));
    }

    match builder.build() {
        Ok(config) => match config.try_deserialize() {
            Ok(settings) => {
                info!("Successfully loaded configuration");
                debug!("Loaded settings: {:?}", settings);
                Ok(settings)
            }
            Err(e) => {
                warn!("Failed to deserialize config, using defaults: {}", e);
                Ok(Settings::default())
            }
        },
        Err(e) => {
            warn!("Failed to load config, using defaults: {}", e);
            Ok(Settings::default())
        }
    }
}

fn is_on_battery(power_path: &str) -> Result<bool, Box<dyn Error>> {
    let entries = fs::read_dir(power_path)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.to_string_lossy().contains("AC") {
            let online_path = path.join("online");
            if let Ok(content) = fs::read_to_string(online_path) {
                return Ok(content.trim() == "0");
            }
        }
    }

    Ok(false)
}

fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging
    env_logger::init();
    println!("Starting Observer...");
    info!("Starting Observer...");

    // Print available cores immediately
    let available_cores = CoreManager::get_available_cores();
    println!(
        "Found {} CPU cores: {:?}",
        available_cores.len(),
        available_cores
    );

    // Load configuration
    let settings = match load_config() {
        Ok(settings) => {
            info!("Loaded configuration: {:?}", settings);
            settings
        }
        Err(e) => {
            warn!("Failed to load config, using defaults: {}", e);
            Settings::default()
        }
    };

    // Setup graceful shutdown handling
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        println!("Ctrl+C received, restoring cores and shutting down...");
        r.store(false, Ordering::SeqCst);
    })?;

    // Also handle other termination signals
    let r2 = running.clone();
    if let Err(e) = ctrlc::set_handler(move || {
        println!("Termination signal received, restoring cores and shutting down...");
        r2.store(false, Ordering::SeqCst);
    }) {
        warn!("Failed to set termination signal handler: {}", e);
    }

    let mut core_manager = CoreManager::new(settings);
    let power_supply_path = "/sys/class/power_supply/";

    info!("Starting main service loop");
    while running.load(Ordering::SeqCst) {
        match is_on_battery(power_supply_path) {
            Ok(on_battery) => {
                let power_state = if on_battery { "battery" } else { "AC" };
                debug!("Current power state: {}", power_state);

                let optimal_cores = core_manager.get_optimal_core_count(on_battery);
                if let Err(e) = core_manager.manage_cpu_cores(optimal_cores) {
                    error!("Failed to manage CPU cores: {}", e);
                }
            }
            Err(e) => {
                error!("Failed to determine power state: {}", e);
            }
        }

        thread::sleep(Duration::from_secs(
            core_manager.settings.check_interval_sec,
        ));
    }

    // Re-enable all cores on shutdown
    info!("Shutdown signal received - restoring all cores...");
    let available_cores = CoreManager::get_available_cores();

    // Force enable all cores directly
    for core_num in available_cores.iter().skip(1) {
        // Skip CPU0
        let online_path = format!("/sys/devices/system/cpu/cpu{}/online", core_num);
        if let Err(e) = fs::write(&online_path, "1") {
            error!("Failed to enable core {} during shutdown: {}", core_num, e);
        } else {
            debug!("Enabled core {} during shutdown", core_num);
        }
    }

    info!("Attempted to restore all cores during shutdown");

    // Verify core states
    for core_num in available_cores.iter().skip(1) {
        let online_path = format!("/sys/devices/system/cpu/cpu{}/online", core_num);
        match fs::read_to_string(&online_path) {
            Ok(state) => {
                if state.trim() != "1" {
                    warn!(
                        "Core {} still appears to be offline after shutdown",
                        core_num
                    );
                }
            }
            Err(e) => error!("Could not verify state of core {}: {}", core_num, e),
        }
    }

    info!("Service shutting down");
    Ok(())
}
