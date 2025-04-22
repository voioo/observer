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
        debug!("Max CPU freq detected: {} KHz, Threshold for E-cores: < {} KHz", max_freq_overall, freq_threshold);

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
                        if max_freq_overall > 0 && freq < freq_threshold {
                            CoreType::Efficiency
                        } else {
                            CoreType::Performance
                        }
                    }
                    None => {
                        warn!("Could not determine max frequency for CPU {}, classifying as Unknown.", core_id);
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

        info!("Detected CPU Topology: {} Physical Cores ({} P-cores, {} E-cores)", final_cores.len(), p_core_count, e_core_count);
        if p_core_count + e_core_count != final_cores.len() {
             warn!("Mismatch in core counts, some cores have Unknown type.");
        }
        for core in &final_cores {
            debug!("  Core {}: Type={:?}, Sibling={}", core.id, core.core_type, core.sibling_id);
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

    #[cfg(target_os = "linux")]
    pub fn get_cores_to_enable(&self, target_count: usize) -> Vec<usize> {
        let total_logical_cores = self.cores.len() * 2; // Assuming SMT2 where siblings != id
        let target_count = target_count.max(1).min(total_logical_cores);

        let mut enabled_cores = std::collections::HashSet::new();

        // Separate cores by type for easier processing
        let p_cores: Vec<&CoreInfo> = self
            .cores
            .iter()
            .filter(|c| c.core_type == CoreType::Performance)
            .collect();
        let e_cores: Vec<&CoreInfo> = self
            .cores
            .iter()
            .filter(|c| c.core_type == CoreType::Efficiency)
            .collect();
        let unknown_cores: Vec<&CoreInfo> = self
            .cores
            .iter()
            .filter(|c| c.core_type == CoreType::Unknown)
            .collect();

        // Helper function to try adding a core and optionally its sibling
        let mut try_add = |core_id: usize, sibling_id: usize, is_p_core: bool| {
            if enabled_cores.len() < target_count {
                enabled_cores.insert(core_id);
            }
            // Add sibling only if needed, target > 1, and it's a P-core or we still need cores
            if enabled_cores.len() < target_count && target_count > 1 && core_id != sibling_id && (is_p_core || enabled_cores.len() < self.num_p_cores * 2) {
                 enabled_cores.insert(sibling_id);
            }
        };

        // 1. Ensure Core 0 is always enabled (find its info)
        if let Some(core0_info) = self.cores.iter().find(|c| c.id == 0) {
            try_add(core0_info.id, core0_info.sibling_id, core0_info.core_type == CoreType::Performance);
        } else {
            // Fallback: If core 0 wasn't in our list (unlikely), just add 0
            if target_count > 0 {
                enabled_cores.insert(0);
            }
        }

        // 2. Fill remaining P-cores and their siblings
        for p_core in p_cores {
            if !enabled_cores.contains(&p_core.id) {
                try_add(p_core.id, p_core.sibling_id, true);
            }
            if enabled_cores.len() >= target_count {
                break;
            }
        }

        // 3. Fill E-cores if needed (prioritize the main core ID first)
        if enabled_cores.len() < target_count {
            for e_core in &e_cores {
                 if enabled_cores.len() < target_count && !enabled_cores.contains(&e_core.id) {
                     enabled_cores.insert(e_core.id);
                 }
                 if enabled_cores.len() >= target_count {
                     break;
                 }
            }
        }
        // 3b. Fill E-core siblings if needed
        if enabled_cores.len() < target_count {
            for e_core in &e_cores {
                 if enabled_cores.len() < target_count && e_core.id != e_core.sibling_id && !enabled_cores.contains(&e_core.sibling_id) {
                     enabled_cores.insert(e_core.sibling_id);
                 }
                 if enabled_cores.len() >= target_count {
                     break;
                 }
            }
        }

        // 4. Fill Unknown cores if still needed (same logic as E-cores)
        if enabled_cores.len() < target_count {
            for u_core in &unknown_cores {
                 if enabled_cores.len() < target_count && !enabled_cores.contains(&u_core.id) {
                     enabled_cores.insert(u_core.id);
                 }
                 if enabled_cores.len() >= target_count {
                     break;
                 }
            }
        }
        if enabled_cores.len() < target_count {
            for u_core in &unknown_cores {
                 if enabled_cores.len() < target_count && u_core.id != u_core.sibling_id && !enabled_cores.contains(&u_core.sibling_id) {
                     enabled_cores.insert(u_core.sibling_id);
                 }
                 if enabled_cores.len() >= target_count {
                     break;
                 }
            }
        }

        let mut final_cores: Vec<usize> = enabled_cores.into_iter().collect();
        final_cores.sort();
        debug!(
            "Targeting {} cores on Linux ({}P, {}E). Enabling cores: {:?}",
            target_count, self.num_p_cores, self.num_e_cores, final_cores
        );
        final_cores
    }

    #[cfg(not(target_os = "linux"))]
    pub fn get_cores_to_enable(&self, _target_count: usize) -> Vec<usize> {
        let target_count = _target_count.max(1);
        // On non-linux, we don't know topology, just return the first N cores.
        // The actual enabling/disabling won't happen anyway.
        let cores: Vec<usize> = (0..target_count).collect();
        debug!(
            "Targeting {} cores on non-Linux, returning simple range: {:?}",
            target_count, cores
        );
        cores
    }
}
