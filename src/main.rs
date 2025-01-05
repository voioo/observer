use log::{debug, error, info, warn};
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::{fs, thread};

use observer::{
    config::{self, Settings},
    core::CoreManager,
    system::power::is_on_battery,
    utils::logging,
};

fn main() -> Result<(), Box<dyn Error>> {
    logging::init();
    println!("Starting Observer...");
    info!("Starting Observer");

    // Print available cores immediately
    let available_cores = CoreManager::get_available_cores();
    println!(
        "Found {} CPU cores: {:?}",
        available_cores.len(),
        available_cores
    );

    let settings = match config::load_config() {
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
        info!("Shutdown signal received, initiating cleanup...");
        r.store(false, Ordering::SeqCst);
    })?;

    let mut core_manager = CoreManager::new(settings);
    let power_supply_path = "/sys/class/power_supply/";

    info!("Starting main service loop");
    while running.load(Ordering::SeqCst) {
        match is_on_battery(power_supply_path) {
            Ok(on_battery) => {
                let optimal_cores = core_manager.get_optimal_core_count(on_battery);
                if let Err(e) = core_manager.manage_cpu_cores(optimal_cores) {
                    error!("Failed to manage CPU cores: {}", e);
                }
            }
            Err(e) => {
                error!("Failed to determine power state: {}", e);
            }
        }

        thread::sleep(Duration::from_secs(settings.check_interval_sec));
    }

    info!("Shutdown signal received - restoring all cores...");
    let available_cores = CoreManager::get_available_cores();

    for core_num in available_cores.iter().skip(1) {
        let online_path = format!("/sys/devices/system/cpu/cpu{}/online", core_num);
        if let Err(e) = fs::write(&online_path, "1") {
            error!("Failed to enable core {} during shutdown: {}", core_num, e);
        } else {
            debug!("Enabled core {} during shutdown", core_num);
        }
    }

    info!("Service shutting down");
    Ok(())
}
