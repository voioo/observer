use log::{debug, warn};

#[cfg(target_os = "linux")]
use std::{fs, path::Path};

#[cfg(target_os = "linux")]
use std::collections::HashMap;

#[cfg(target_os = "linux")]
use log::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreType {
    Performance,
    Efficiency,
    Unknown, // Fallback if detection fails
}

#[derive(Debug, Clone)]
pub struct CoreInfo {
    pub id: usize,
    pub sibling_id: usize, // ID of the other thread in the SMT pair
    pub core_type: CoreType,
}

#[derive(Debug, Clone)]
pub struct CPUTopology {
    pub cores: Vec<CoreInfo>, // Info for each physical core (one entry per pair)
    pub num_p_cores: usize,   // Count of physical Performance cores
    pub num_e_cores: usize,   // Count of physical Efficiency cores
}

impl Default for CPUTopology {
    fn default() -> Self {
        Self::new()
    }
}

impl CPUTopology {
    #[cfg(target_os = "linux")]
    pub fn new() -> Self {
        let cpu_path = Path::new("/sys/devices/system/cpu");
        let mut core_details = HashMap::new(); // Map core_id -> (Option<sibling_id>, Option<max_freq_khz>)
        let mut max_freq_overall = 0;

        // First pass: Discover cores, siblings, and max frequencies
        for i in 0.. {
            let core_dir = cpu_path.join(format!("cpu{}", i));
            if !core_dir.exists() {
                break; // Assume we found all cores
            }

            let mut sibling_id = Some(i); // Default sibling to self if not found
            let mut max_freq = None;

            // Read siblings
            let siblings_path = core_dir.join("topology/thread_siblings_list");
            if let Ok(siblings_str) = fs::read_to_string(siblings_path) {
                let siblings: Vec<usize> = siblings_str
                    .trim()
                    .split(',')
                    .filter_map(|s| s.parse().ok())
                    .collect();
                // Find the sibling that isn't the current core 'i'
                if let Some(other_sibling) = siblings.iter().find(|&&s| s != i) {
                    sibling_id = Some(*other_sibling);
                } else if siblings.len() == 1 && siblings[0] == i {
                    // Core without SMT sibling
                    sibling_id = Some(i);
                }
            }

            // Read max frequency
            let freq_path = core_dir.join("cpufreq/scaling_max_freq");
            if let Ok(freq_str) = fs::read_to_string(freq_path) {
                if let Ok(freq_khz) = freq_str.trim().parse::<usize>() {
                    max_freq = Some(freq_khz);
                    if freq_khz > max_freq_overall {
                        max_freq_overall = freq_khz;
                    }
                }
            }

            core_details.insert(i, (sibling_id, max_freq));
        }

        if core_details.is_empty() {
            warn!("Could not read any CPU details from /sysfs. Topology unavailable.");
            return CPUTopology {
                cores: Vec::new(),
                num_p_cores: 0,
                num_e_cores: 0,
            };
        }

        // Determine frequency threshold for P vs E cores (e.g., 75% of max)
        let freq_threshold = (max_freq_overall as f64 * 0.75) as usize;
        debug!(
            "Max CPU freq detected: {} KHz, Threshold for E-cores: < {} KHz",
            max_freq_overall, freq_threshold
        );

        let mut final_cores = Vec::new();
        let mut processed_ids = std::collections::HashSet::new(); // Keep track of processed core IDs
        let mut p_core_count = 0;
        let mut e_core_count = 0;

        // Second pass: Classify cores and create CoreInfo, avoiding duplicates for SMT pairs
        let mut core_ids: Vec<usize> = core_details.keys().cloned().collect();
        core_ids.sort(); // Process in order

        for core_id in core_ids {
            if processed_ids.contains(&core_id) {
                continue; // Already processed as part of a pair
            }

            if let Some((sibling_opt, freq_opt)) = core_details.get(&core_id) {
                let sibling_id = sibling_opt.unwrap_or(core_id); // Default to self if None

                let core_type = match freq_opt {
                    Some(freq) => {
                        // Compare dereferenced freq with freq_threshold
                        if max_freq_overall > 0 && *freq < freq_threshold {
                            CoreType::Efficiency
                        } else {
                            CoreType::Performance
                        }
                    }
                    None => {
                        warn!(
                            "Could not determine max frequency for CPU {}, classifying as Unknown.",
                            core_id
                        );
                        CoreType::Unknown
                    }
                };

                let core_info = CoreInfo {
                    id: core_id,
                    sibling_id,
                    core_type,
                };
                final_cores.push(core_info);

                match core_type {
                    CoreType::Performance => p_core_count += 1,
                    CoreType::Efficiency => e_core_count += 1,
                    CoreType::Unknown => { /* Don't count Unknown towards P/E totals */ }
                }

                // Mark both core and its sibling as processed
                processed_ids.insert(core_id);
                if sibling_id != core_id {
                    processed_ids.insert(sibling_id);
                }
            }
        }

        info!(
            "Detected CPU Topology: {} Physical Cores ({} P-cores, {} E-cores)",
            final_cores.len(),
            p_core_count,
            e_core_count
        );
        if p_core_count + e_core_count != final_cores.len() {
            warn!("Mismatch in core counts, some cores have Unknown type.");
        }
        for core in &final_cores {
            debug!(
                "  Core {}: Type={:?}, Sibling={}",
                core.id, core.core_type, core.sibling_id
            );
        }

        CPUTopology {
            cores: final_cores,
            num_p_cores: p_core_count,
            num_e_cores: e_core_count,
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn new() -> Self {
        warn!("CPU topology detection is only supported on Linux. Assuming no specific topology.");
        CPUTopology {
            cores: Vec::new(), // Return empty topology on non-Linux
            num_p_cores: 0,
            num_e_cores: 0,
        }
    }
}
