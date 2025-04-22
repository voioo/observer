use log::{debug, error, info, warn};
use std::error::Error;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

use crate::utils::logging;

mod config;
mod core;
mod system;
mod utils;

fn main() -> Result<(), Box<dyn Error>> {
    logging::init();
    println!("Starting Observer...");
    info!("Starting Observer");

    #[cfg(target_os = "linux")]
    let available_cores = crate::core::CoreManager::get_available_cores()?;
    #[cfg(target_os = "linux")]
    println!(
        "Found {} CPU cores: {:?}",
        available_cores.len(),
        available_cores
    );

    let settings = match crate::config::load_config() {
        Ok(s) => {
            info!("Loaded configuration: {:?}", s.clone());
            s
        }
        Err(e) => {
            warn!("Failed to load config, using defaults: {}", e);
            crate::config::Settings::default()
        }
    };

    info!("Loaded configuration: {:?}", settings);

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        info!("Shutdown signal received, exiting...");
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    info!("Initializing Core Manager...");
    let mut core_manager = crate::core::CoreManager::new(settings.clone())?;
    info!("Core Manager initialized successfully.");

    info!("Starting main loop...");

    let check_interval = settings.check_interval_sec;
    #[cfg(target_os = "linux")]
    let power_supply_path = "/sys/class/power_supply/";

    info!("Starting main service loop");
    while running.load(Ordering::SeqCst) {
        debug!("Main loop iteration");

        #[cfg(target_os = "linux")]
        let power_state_result = crate::system::power::get_power_state(power_supply_path);
        #[cfg(not(target_os = "linux"))]
        let power_state_result = Ok(crate::system::power::PowerState::AC);

        match power_state_result {
            Ok(power_state) => {
                let on_battery = power_state == crate::system::power::PowerState::Battery;
                debug!(
                    "Current power state: {:?}, On Battery: {}",
                    power_state, on_battery
                );

                let optimal_cores = core_manager.get_optimal_core_count(on_battery)?;
                debug!("Optimal core count: {}", optimal_cores);

                if let Err(e) = core_manager.manage_cpu_cores(optimal_cores) {
                    error!("Failed to manage CPU cores: {}", e);
                }
            }
            Err(e) => {
                error!("Failed to get power state: {}. Assuming AC power.", e);
            }
        }

        debug!("Sleeping for {} seconds", check_interval);
        thread::sleep(Duration::from_secs(check_interval));
    }

    info!("Service shutting down");
    Ok(())
}
