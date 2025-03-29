use crate::config::{ModifiersArray};
use crate::device::SavedDevice;
use crate::{SharedDeviceState, SharedStateFlag}; // Import shared types
use crate::util::{self, merge_u8_into_u16, read_bit, set_bit, ReportFormat, MAX_REPORT_SIZE};
use log::{debug, error, info, trace, warn};
use hidapi::{HidApi, HidDevice, HidError};
use std::{
    sync::{Arc, Condvar, Mutex},
    thread,
    time::Duration,
};

// Constants for HID communication
pub const VENDOR_ID_FILTER: u16 = 0x3344; // Assuming Virpil VID
const WORKER_SLEEP_MS: u64 = 100; // Reduced sleep time for better responsiveness


#[derive(Clone)]
struct DeviceWorkerInfo {
    config: SavedDevice,
    format: ReportFormat,
}

// Structure to hold data passed to the worker thread
// Clone Arcs for shared state, clone config data needed
struct WorkerData {
    run_state: SharedStateFlag,
    sources_info: Vec<DeviceWorkerInfo>,
    receivers_info: Vec<DeviceWorkerInfo>,
    shift_modifiers: ModifiersArray,
    source_states_shared: Vec<SharedDeviceState>,
    receiver_states_shared: Vec<SharedDeviceState>,
    final_shift_state_shared: SharedDeviceState,
}

// Main function to spawn the worker thread
// Now part of ShiftTool impl block
impl crate::ShiftTool {
    pub(crate) fn spawn_worker(&mut self) -> bool {
        info!("Attempting to spawn HID worker thread...");

        let mut sources_info: Vec<DeviceWorkerInfo> = Vec::new();
        for (i, source_config) in self.config.data.sources.iter().enumerate() {

            // 1. Find the corresponding VpcDevice in the current device_list
            //    This is needed to get the firmware string.
            let device_idx = crate::device::find_device_index_for_saved(
                &self.device_list, // The list of currently detected devices
                source_config,     // The config for the i-th source slot
            );

            // 2. Get the firmware string from the found VpcDevice
            let firmware_str = if device_idx != 0 && device_idx < self.device_list.len() {
                // Successfully found the device in the current list
                self.device_list[device_idx].firmware.to_string() // Access the firmware field
            } else {
                // Device not found (index 0 is default/placeholder) or list issue
                warn!("Source device {} not found in current list for format determination.", i);
                "".to_string() // Use empty string if not found
            };

            let name_str = if device_idx != 0 && device_idx < self.device_list.len() {
                // Successfully found the device in the current list
                self.device_list[device_idx].name.to_string() // Access the firmware field
            } else {
                // Device not found (index 0 is default/placeholder) or list issue
                warn!("Source device {} not found in current list for format determination.", i);
                "".to_string() // Use empty string if not found
            };

            // 3. Call determine_report_format with the firmware string
            //    This function (from src/util.rs) contains the logic
            //    to check dates or patterns and return the correct format.
            let determined_format: ReportFormat = util::determine_report_format(&name_str, &firmware_str);

            // 4. Log the result for debugging
            info!(
                "Determined report format {:?} for source {} (Firmware: '{}')",
                determined_format, // Log the whole struct (uses Debug derive)
                i,
                firmware_str
            );

            // 5. Store the result along with the config in DeviceWorkerInfo
            sources_info.push(DeviceWorkerInfo {
                config: source_config.clone(), // Clone the config part
                format: determined_format,     // Store the determined format
            });
        }

        let mut receivers_info: Vec<DeviceWorkerInfo> = Vec::new();
        for (i, receiver_config) in self.config.data.receivers.iter().enumerate() {
            let device_idx = crate::device::find_device_index_for_saved(
                &self.device_list,
                receiver_config,
            );
            let firmware_str = if device_idx != 0 && device_idx < self.device_list.len() {
                self.device_list[device_idx].firmware.to_string()
            } else {
                warn!("Receiver device {} not found in current list for format determination.", i);
                "".to_string()
            };
            let name_str = if device_idx != 0 && device_idx < self.device_list.len() {
                self.device_list[device_idx].name.to_string()
            } else {
                warn!("Receiver device {} not found in current list for format determination.", i);
                "".to_string()
            };

            let determined_format: ReportFormat = util::determine_report_format(&name_str, &firmware_str);

            info!(
                "Determined report format {:?} for receiver {} (Firmware: '{}')",
                determined_format,
                i,
                firmware_str
            );

            receivers_info.push(DeviceWorkerInfo {
                config: receiver_config.clone(),
                format: determined_format,
            });
        }


        // Clone data needed by the thread
        let worker_data = WorkerData {
            run_state: self.thread_state.clone(),
            sources_info,
            receivers_info,
            shift_modifiers: self.config.data.shift_modifiers, // Copy (it's Copy)
            source_states_shared: self.source_states.clone(),
            receiver_states_shared: self.receiver_states.clone(),
            final_shift_state_shared: self.shift_state.clone(),
        };

        // Spawn the thread
        thread::spawn(move || {
            // Create HidApi instance *within* the thread
            match HidApi::new() { // Use new() which enumerates internally
                Ok(hidapi) => {
                    info!("HidApi created successfully in worker thread.");
                    // Filter devices *within* the thread if needed, though opening by VID/PID/SN is primary
                    // hidapi.add_devices(VENDOR_ID_FILTER, 0).ok(); // Optional filtering

                    run_hid_worker_loop(hidapi, worker_data);
                }
                Err(e) => {
                    error!("Failed to create HidApi in worker thread: {}", e);
                    // How to signal failure back? Could use another shared state.
                    // For now, thread just exits.
                }
            }
        });

        info!("HID worker thread spawn initiated.");
        true // Indicate spawn attempt was made
    }

    // Cleanup actions when the worker is stopped from the UI
    pub(crate) fn stop_worker_cleanup(&mut self) {
        info!("Performing worker stop cleanup...");
        // Reset shared states displayed in the UI
        let reset_state = |state_arc: &SharedDeviceState| {
            if let Ok(mut state) = state_arc.lock() {
                *state = 0;
            }
            // No need to notify condvar if only UI reads it
        };

        self.source_states.iter().for_each(reset_state);
        self.receiver_states.iter().for_each(reset_state);
        reset_state(&self.shift_state);

        // Mark all devices as inactive in the UI list
        for device in self.device_list.iter_mut() {
            device.active = false;
        }
        info!("Worker stop cleanup finished.");
    }
}


/// Opens HID devices based on the provided configuration and format info.
///
/// Iterates through the `device_infos`, attempts to open each device using
/// VID, PID, and Serial Number from the `config` field. Sets non-blocking mode.
///
/// Returns a Vec where each element corresponds to an input `DeviceWorkerInfo`.
/// Contains `Some(HidDevice)` on success, or `None` if the device couldn't be
/// opened, wasn't configured (VID/PID=0), or failed to set non-blocking mode.
fn open_hid_devices(
    hidapi: &HidApi,
    device_infos: &[DeviceWorkerInfo], // Accepts a slice of the new struct
) -> Vec<Option<HidDevice>> {
    let mut devices = Vec::with_capacity(device_infos.len());

    // Iterate through the DeviceWorkerInfo structs
    for (i, info) in device_infos.iter().enumerate() {
        // Use info.config to get the device identifiers
        let config = &info.config;

        // Skip if device is not configured (VID/PID are zero)
        if config.vendor_id == 0 || config.product_id == 0 {
            log::trace!("Skipping opening device slot {} (unconfigured).", i);
            devices.push(None); // Placeholder for unconfigured slot
            continue;
        }

        // Attempt to open the device
        match hidapi.open(
            config.vendor_id,
            config.product_id,
        ) {
            Ok(device) => {
                // Log success with format info for context
                log::info!(
                    "Successfully opened device slot {}: VID={:04X}, PID={:04X}, SN='{}', Format='{}'",
                    i, config.vendor_id, config.product_id, config.serial_number, info.format.name // Log format name
                );

                // Attempt to set non-blocking mode
                if let Err(e) = device.set_blocking_mode(false) {
                    log::error!(
                        "Failed to set non-blocking mode for device slot {}: {:?}. Treating as open failure.",
                        i, e
                    );
                    // Decide if this is fatal: Yes, treat as failure if non-blocking fails
                    devices.push(None);
                } else {
                    // Successfully opened and set non-blocking
                    devices.push(Some(device));
                }
            }
            Err(e) => {
                // Log failure to open
                log::warn!(
                    "Failed to open device slot {}: VID={:04X}, PID={:04X}, SN='{}': {:?}",
                    i, config.vendor_id, config.product_id, config.serial_number, e
                );
                devices.push(None); // Push None on failure
            }
        }
    }
    devices
}


// The core worker loop logic
fn run_hid_worker_loop(hidapi: HidApi, data: WorkerData) {
    log::info!("HID worker loop starting.");

    // --- Device Opening ---
    // Open sources and receivers, keeping track of which ones succeeded
    let mut source_devices = open_hid_devices(&hidapi, &data.sources_info);
    let mut receiver_devices = open_hid_devices(&hidapi, &data.receivers_info);

    // Buffers for HID reports
    let mut read_buffer = [0u8; MAX_REPORT_SIZE];
    let mut write_buffer = [0u8; MAX_REPORT_SIZE]; // Buffer for calculated output

    let &(ref run_lock, ref run_cvar) = &*data.run_state;

    loop {
        // --- Check Run State ---
        let should_run = { // Scope for mutex guard
            match run_lock.lock() {
                Ok(guard) => *guard,
                Err(poisoned) => {
                    error!("Run state mutex poisoned in worker loop!");
                    false
                }
            }
        };

        if !should_run {
            info!("Stop signal received, exiting worker loop.");
            break; // Exit the loop
        }

        // --- Read from Source Devices ---
        let mut current_source_states: Vec<Option<u16>> = vec![None; source_devices.len()];

        for (i, device_opt) in source_devices.iter_mut().enumerate() {
            if let Some(device) = device_opt {
                let source_info = &data.sources_info[i];
                let source_format = source_info.format;
                read_buffer[0] = source_format.report_id;

                // Attempt to read feature report
                match device.get_feature_report(&mut read_buffer) {
                    Ok(bytes_read) => {
                        if let Some(state_val) = source_format.unpack_state(&read_buffer[0..bytes_read]) {
                            trace!("Worker: Unpacked state {} from source {}", state_val, i);
                            current_source_states[i] = Some(state_val);
                            // Update shared state for UI
                            if let Some(shared_state) = data.source_states_shared.get(i) {
                                if let Ok(mut guard) = shared_state.lock() { *guard = state_val; }
                                else { log::error!("Worker: Mutex poisoned for source_states_shared[{}]!", i); }
                            }
                        } else {
                            // unpack_state returned None (e.g., wrong ID, too short)
                            log::warn!("Worker: Failed to unpack state from source {} (bytes read: {}) using format '{}'", i, bytes_read, source_format.name);
                            current_source_states[i] = None;
                            if let Some(shared_state) = data.source_states_shared.get(i) {
                                if let Ok(mut guard) = shared_state.lock() { *guard = 0; } // Reset UI
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("Worker: Error reading from source {}: {:?}. Attempting reopen.", i, e);
                        current_source_states[i] = None;
                        if let Some(shared_state) = data.source_states_shared.get(i) {
                            if let Ok(mut guard) = shared_state.lock() { *guard = 0; }
                        }
                        // Reopen logic using source_info.config
                        log::debug!("Worker: Attempting to reopen source[{}]...", i);
                        *device_opt = hidapi.open_serial(
                            source_info.config.vendor_id,
                            source_info.config.product_id,
                            &source_info.config.serial_number,
                        ).ok().and_then(|d| d.set_blocking_mode(false).ok().map(|_| d)); // Simplified reopen
                        if device_opt.is_some() { log::info!("Worker: Reopen successful for source[{}].", i); }
                        else { log::warn!("Worker: Reopen failed for source[{}].", i); }
                    }
                }
            } else {
                // Device was not opened initially or failed reopen
                current_source_states[i] = None;
                if let Some(shared_state) = data.source_states_shared.get(i) {
                    if let Ok(mut guard) = shared_state.lock() { *guard = 0; } // Reset UI state
                }
            }
        }

        // --- 3. Calculate Final State based on Rules ---
        let mut final_state: u16 = 0;
        for bit_pos in 0..8u8 {
            let mut relevant_values: Vec<bool> = Vec::new();
            for (source_idx, state_opt) in current_source_states.iter().enumerate() {
                if data.sources_info[source_idx].config.state_enabled[bit_pos as usize] {
                    relevant_values.push(state_opt.map_or(false, |s| util::read_bit(s, bit_pos)));
                }
            }
            if !relevant_values.is_empty() {
                let modifier = data.shift_modifiers[bit_pos as usize];
                let result_bit = match modifier {
                    crate::config::ShiftModifiers::OR => relevant_values.iter().any(|&v| v),
                    crate::config::ShiftModifiers::AND => relevant_values.iter().all(|&v| v),
                    crate::config::ShiftModifiers::XOR => relevant_values.iter().fold(false, |acc, &v| acc ^ v),
                };
                if result_bit { final_state |= 1 << bit_pos; }
            }
        }
        // Update shared final state for UI
        if let Ok(mut guard) = data.final_shift_state_shared.lock() {
            *guard = final_state;
        }
        // --- End Calculate Final State ---

        // --- 4. Write to Receiver Devices ---
        for (i, device_opt) in receiver_devices.iter_mut().enumerate() {
            if let Some(device) = device_opt {
                let receiver_info = &data.receivers_info[i];
                let receiver_format = receiver_info.format;

                // --- 4a. Send Zero State Report First ---
                let zero_buffer_slice = receiver_format.pack_state(&mut write_buffer, 0);
                if zero_buffer_slice.is_empty() { /* handle error */ continue; }

                log::trace!("Worker: Sending zero state reset ({} bytes) to receiver[{}] using format '{}'", receiver_format.total_size, i, receiver_format.name);
                match device.send_feature_report(zero_buffer_slice) {
                    Ok(_) => {
                        log::trace!("Worker: Zero state sent successfully to receiver[{}].", i);

                        // --- 4b. If Zero Send OK, Prepare and Send Actual State ---
                        let mut state_to_send = final_state; // Start with the globally calculated state

                        // Apply receiver's enabled mask
                        for bit_pos in 0..8u8 {
                            if !receiver_info.config.state_enabled[bit_pos as usize] {
                                state_to_send &= !(1 << bit_pos);
                            }
                        }

                        // --- Start: Read receiver's current state and merge ---
                        let mut receiver_current_state: u16 = 0; // Default to 0 if read fails
                        read_buffer[0] = receiver_format.report_id; // Set ID for reading receiver

                        log::trace!("Worker: Reading current state from receiver[{}] before merge.", i);
                        match device.get_feature_report(&mut read_buffer) {
                            Ok(bytes_read) => {
                                if let Some(current_state) = receiver_format.unpack_state(&read_buffer[0..bytes_read]) {
                                    log::trace!("Worker: Receiver[{}] current unpacked state: {}", i, current_state);
                                    receiver_current_state = current_state;
                                } else {
                                    log::warn!("Worker: Failed to unpack current state from receiver {} (bytes read: {}) using format '{}'. Merge will use 0.", i, bytes_read, receiver_format.name);
                                }
                            }
                            Err(e_read) => {
                                // Log error reading current state, but proceed with merge using 0
                                log::warn!("Worker: Error reading current state from receiver[{}]: {:?}. Merge will use 0.", i, e_read);
                                // Note: Don't attempt reopen here, as we are about to send anyway.
                                // If send fails later, reopen will be attempted then.
                            }
                        }
                        state_to_send |= receiver_current_state; // Merge
                        // --- End Read current state ---

                        // Use pack_state to prepare the buffer slice with the potentially merged state
                        let actual_buffer_slice = receiver_format.pack_state(
                            &mut write_buffer,
                            state_to_send, // Use the final (potentially merged) state
                        );

                        if actual_buffer_slice.is_empty() { /* handle pack error */ continue; }

                        log::debug!(
                            "Worker: Attempting send final state to receiver[{}], state: {}, buffer ({} bytes): {:02X?}",
                            i, state_to_send, receiver_format.total_size, actual_buffer_slice
                        );

                        // Send the actual calculated/merged state
                        match device.send_feature_report(actual_buffer_slice) {
                            Ok(_) => {
                                log::debug!("Worker: Final state send to receiver[{}] successful.", i);
                                // Update shared state for UI with the state we just sent
                                if let Some(shared_state) = data.receiver_states_shared.get(i) {
                                    if let Ok(mut guard) = shared_state.lock() {
                                        *guard = state_to_send; // Update with the sent state
                                    } else {
                                        if let Some(shared_state) = data.receiver_states_shared.get(i) {
                                            match shared_state.lock() {
                                                Ok(mut guard) => *guard = 0,
                                                Err(poisoned) => {
                                                    log::error!("Mutex for receiver_states_shared[{}] poisoned! Recovering and resetting.", i);
                                                    *poisoned.into_inner() = 0;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e_actual) => {
                                // ... (error handling, reopen logic for send failure) ...
                                log::warn!("Worker: Error sending final state to receiver[{}]: {:?}", i, e_actual);
                                if let Some(shared_state) = data.receiver_states_shared.get(i) {
                                    match shared_state.lock() {
                                        Ok(mut guard) => *guard = 0,
                                        Err(poisoned) => {
                                            log::error!("Mutex for receiver_states_shared[{}] poisoned! Recovering and resetting.", i);
                                            *poisoned.into_inner() = 0;
                                        }
                                    }
                                }

                                log::debug!("Worker: Attempting to reopen receiver[{}] after final-send failure...", i);
                                *device_opt = hidapi.open(
                                    data.receivers_info[i].config.vendor_id,
                                    data.receivers_info[i].config.product_id,
                                ).ok().and_then(|d| {
                                    d.set_blocking_mode(false).ok()?;
                                    Some(d)
                                });

                                if device_opt.is_none() {
                                    log::warn!("Reopen failed for receiver {}.", i);
                                } else {
                                    log::info!("Reopen successful for receiver {}.", i);
                                }
                            }
                        } // End match send actual state
                    } // End Ok for zero send
                    Err(e_zero) => {
                        // Handle error sending the zero state reset
                        log::warn!("Worker: Error sending zero state reset to receiver[{}]: {:?}", i, e_zero);
                        // Reset UI state, attempt reopen
                        if let Some(shared_state) = data.receiver_states_shared.get(i) {
                            if let Ok(mut guard) = shared_state.lock() { *guard = 0; }
                        }
                        log::debug!("Worker: Attempting to reopen receiver[{}] after zero-send failure...", i);
                        *device_opt = hidapi.open( data.receivers_info[i].config.vendor_id,
                                                   data.receivers_info[i].config.product_id
                        ).ok().and_then(|d| {
                            d.set_blocking_mode(false).ok()?;
                            Some(d)
                        });
                        if device_opt.is_none() {
                            log::warn!("Reopen failed for receiver {}.", i);
                        } else {
                            log::info!("Reopen successful for receiver {}.", i);
                        }
                    } // End Err for zero send
                }
            } else {
                // Device not open, reset UI state
                if let Some(shared_state) = data.receiver_states_shared.get(i) {
                    if let Ok(mut guard) = shared_state.lock() { *guard = 0; }
                }
            }
        }

        // --- Sleep ---
        thread::sleep(Duration::from_millis(WORKER_SLEEP_MS));
    } // End loop

    // --- Cleanup before thread exit ---
    log::info!("Worker loop finished. Performing cleanup...");
    for (i, device_opt) in receiver_devices.iter_mut().enumerate() {
        if let Some(device) = device_opt {
            let receiver_info = &data.receivers_info[i];
            let receiver_format = receiver_info.format;

            // --- 4a. Send Zero State Report First ---
            let zero_buffer_slice = receiver_format.pack_state(&mut write_buffer, 0);
            if zero_buffer_slice.is_empty() { /* handle error */ continue; }

            log::trace!("Worker: Sending zero state reset ({} bytes) to receiver[{}] using format '{}'", receiver_format.total_size, i, receiver_format.name);
            match device.send_feature_report(zero_buffer_slice) {
                Ok(_) => {
                    log::trace!("Worker: Zero state sent successfully to receiver[{}].", i);
                    if let Some(shared_state) = data.receiver_states_shared.get(i) {
                        if let Ok(mut guard) = shared_state.lock() { *guard = 0; }
                    }
                }
                Err(e_actual) => {
                    if let Some(shared_state) = data.receiver_states_shared.get(i) {
                        if let Ok(mut guard) = shared_state.lock() { *guard = 0; }
                    }
                }
            }
        }
    }
    log::info!("Worker thread cleanup complete. Exiting.");
}
