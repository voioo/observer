mod settings;

use config::{Config, ConfigError, File};
use log::{debug, info, warn};
pub use settings::Settings;

pub fn load_config() -> Result<Settings, ConfigError> {
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
