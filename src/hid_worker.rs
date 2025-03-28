use crate::config::{ModifiersArray};
use crate::device::SavedDevice;
use crate::{SharedDeviceState, SharedStateFlag}; // Import shared types
use crate::util::{merge_u8_into_u16, read_bit, set_bit};
use hidapi::{HidApi, HidDevice, HidError};
use std::{
    sync::{Arc, Condvar, Mutex},
    thread,
    time::Duration,
};

// Constants for HID communication
const FEATURE_REPORT_ID: u8 = 4;
const REPORT_BUFFER_SIZE: usize = 19; // 1 byte ID + 64 bytes data
pub const VENDOR_ID_FILTER: u16 = 0x3344; // Assuming Virpil VID
const WORKER_SLEEP_MS: u64 = 100; // Reduced sleep time for better responsiveness

// Structure to hold data passed to the worker thread
// Clone Arcs for shared state, clone config data needed
struct WorkerData {
    run_state: SharedStateFlag,
    sources_config: Vec<SavedDevice>,
    receivers_config: Vec<SavedDevice>,
    shift_modifiers: ModifiersArray,
    source_states_shared: Vec<SharedDeviceState>,
    receiver_states_shared: Vec<SharedDeviceState>,
    final_shift_state_shared: SharedDeviceState,
}

// Main function to spawn the worker thread
// Now part of ShiftTool impl block
impl crate::ShiftTool {
    pub(crate) fn spawn_worker(&mut self) -> bool {
        log::info!("Attempting to spawn HID worker thread...");

        // Clone data needed by the thread
        let worker_data = WorkerData {
            run_state: self.thread_state.clone(),
            sources_config: self.config.data.sources.clone(),
            receivers_config: self.config.data.receivers.clone(),
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
                    log::info!("HidApi created successfully in worker thread.");
                    // Filter devices *within* the thread if needed, though opening by VID/PID/SN is primary
                    // hidapi.add_devices(VENDOR_ID_FILTER, 0).ok(); // Optional filtering

                    run_hid_worker_loop(hidapi, worker_data);
                }
                Err(e) => {
                    log::error!("Failed to create HidApi in worker thread: {}", e);
                    // How to signal failure back? Could use another shared state.
                    // For now, thread just exits.
                }
            }
        });

        log::info!("HID worker thread spawn initiated.");
        true // Indicate spawn attempt was made
    }

    // Cleanup actions when the worker is stopped from the UI
    pub(crate) fn stop_worker_cleanup(&mut self) {
        log::info!("Performing worker stop cleanup...");
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
        log::info!("Worker stop cleanup finished.");
    }
}


// Helper to open devices, returns Result for better error handling
fn open_hid_devices(
    hidapi: &HidApi,
    device_configs: &[SavedDevice],
) -> Vec<Option<HidDevice>> { // Return Option<HidDevice> to represent open failures
    let mut devices = Vec::with_capacity(device_configs.len());
    for config in device_configs {
        if config.vendor_id == 0 || config.product_id == 0 {
            log::warn!("Skipping device with zero VID/PID in config.");
            devices.push(None); // Placeholder for unconfigured/invalid device
            continue;
        }
        match hidapi.open(
            config.vendor_id,
            config.product_id
        ) {
            Ok(device) => {
                log::info!(
                    "Successfully opened device: VID={:04x}, PID={:04x}, SN={}",
                    config.vendor_id, config.product_id, config.serial_number
                );
                // Set non-blocking mode
                if let Err(e) = device.set_blocking_mode(false) {
                    log::error!("Failed to set non-blocking mode: {}", e);
                    // Decide if this is fatal for this device
                    devices.push(None); // Treat as failure if non-blocking fails
                } else {
                    devices.push(Some(device));
                }
            }
            Err(e) => {
                log::warn!(
                    "Failed to open device VID={:04x}, PID={:04x}, SN={}: {}",
                    config.vendor_id, config.product_id, config.serial_number, e
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
    let mut source_devices = open_hid_devices(&hidapi, &data.sources_config);
    let mut receiver_devices = open_hid_devices(&hidapi, &data.receivers_config);

    // Buffers for HID reports
    let mut read_buffer = [0u8; REPORT_BUFFER_SIZE];
    let mut write_buffer = [0u8; REPORT_BUFFER_SIZE]; // Buffer for calculated output

    let &(ref run_lock, ref run_cvar) = &*data.run_state;

    loop {
        // --- Check Run State ---
        let should_run = { // Scope for mutex guard
            match run_lock.lock() {
                Ok(guard) => *guard,
                Err(poisoned) => {
                    log::error!("Run state mutex poisoned in worker loop!");
                    false
                }
            }
        };

        if !should_run {
            log::info!("Stop signal received, exiting worker loop.");
            break; // Exit the loop
        }

        // --- Read from Source Devices ---
        let mut current_source_states: Vec<Option<u16>> = vec![None; source_devices.len()];
        read_buffer[0] = FEATURE_REPORT_ID; // Set report ID for reading

        for (i, device_opt) in source_devices.iter_mut().enumerate() {
            if let Some(device) = device_opt {
                // Attempt to read feature report
                match device.get_feature_report(&mut read_buffer) {
                    Ok(bytes_read) if bytes_read >= 3 => { // Need at least ID + 2 bytes data
                        let state_val = merge_u8_into_u16(read_buffer[1], read_buffer[2]);
                        current_source_states[i] = Some(state_val);
                        // Update shared state for UI
                        if let Some(shared_state) = data.source_states_shared.get(i) {
                            if let Ok(mut guard) = shared_state.lock() {
                                log::debug!("Worker: Updating source_states_shared[{}] from {} to {}", i, *guard, state_val);
                                *guard = state_val;
                            }
                        }
                    }
                    Ok(bytes_read) => { // Read ok, but not enough data?
                        log::warn!("Source {} read only {} bytes for report {}.", i, bytes_read, FEATURE_REPORT_ID);
                        current_source_states[i] = None; // Treat as no data
                        if let Some(shared_state) = data.source_states_shared.get(i) {
                            if let Ok(mut guard) = shared_state.lock() { *guard = 0; } // Reset UI state
                        }
                    }
                    Err(e) => {
                        log::warn!("Error reading from source {}: {}. Attempting reopen.", i, e);
                        current_source_states[i] = None; // Clear state on error
                        if let Some(shared_state) = data.source_states_shared.get(i) {
                            if let Ok(mut guard) = shared_state.lock() { *guard = 0; } // Reset UI state
                        }
                        // Attempt to reopen the device
                        *device_opt = hidapi.open(
                            data.sources_config[i].vendor_id,
                            data.sources_config[i].product_id
                        ).ok().and_then(|d| { d.set_blocking_mode(false).ok()?; Some(d) }); // Re-open and set non-blocking

                        if device_opt.is_none() {
                            log::warn!("Reopen failed for source {}.", i);
                        } else {
                            log::info!("Reopen successful for source {}.", i);
                        }
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

        // --- Calculate Final State based on Rules ---
        let mut final_state: u16 = 0;
        write_buffer.fill(0); // Clear write buffer
        write_buffer[0] = FEATURE_REPORT_ID;

        for bit_pos in 0..8u8 {
            let mut relevant_values: Vec<bool> = Vec::new();
            for (source_idx, state_opt) in current_source_states.iter().enumerate() {
                // Check if this source is enabled for this bit
                if data.sources_config[source_idx].state_enabled[bit_pos as usize] {
                    if let Some(state_val) = state_opt {
                        relevant_values.push(read_bit(*state_val, bit_pos));
                    } else {
                        // How to handle missing data? Assume false? Or skip?
                        // Assuming false if device errored or didn't report
                        relevant_values.push(false);
                    }
                }
            }

            if !relevant_values.is_empty() {
                let modifier = data.shift_modifiers[bit_pos as usize];
                let result_bit = match modifier {
                    crate::config::ShiftModifiers::OR => relevant_values.iter().any(|&v| v),
                    crate::config::ShiftModifiers::AND => relevant_values.iter().all(|&v| v),
                    crate::config::ShiftModifiers::XOR => relevant_values.iter().fold(false, |acc, &v| acc ^ v),
                };

                // Set the corresponding bit in the final state and write buffer
                if result_bit {
                    final_state |= 1 << bit_pos;
                    // Assuming the state maps directly to bytes 1 and 2
                    write_buffer[1] = final_state as u8; // Low byte
                    write_buffer[2] = (final_state >> 8) as u8; // High byte
                }
            }
        }

        // Update shared final state for UI
        if let Ok(mut guard) = data.final_shift_state_shared.lock() {
            *guard = final_state;
        }

        // --- Write to Receiver Devices ---
        let zero_buffer: [u8; REPORT_BUFFER_SIZE] = {
            let mut buf = [0u8; REPORT_BUFFER_SIZE];
            buf[0] = FEATURE_REPORT_ID; // Set Report ID 4
            // All other bytes (1-18) remain 0 for the zero state
            buf
        };

        for (i, device_opt) in receiver_devices.iter_mut().enumerate() {
            if let Some(device) = device_opt {
                match device.send_feature_report(&zero_buffer) {
                    Ok(_) => {
                        // Create a temporary buffer potentially filtered by receiver's enabled bits
                        let mut filtered_write_buffer = write_buffer; // Copy base calculated state
                        let mut filtered_final_state = final_state;

                        // Apply receiver's enabled mask
                        for bit_pos in 0..8u8 {
                            if !data.receivers_config[i].state_enabled[bit_pos as usize] {
                                // If this bit is disabled for this receiver, force it to 0
                                filtered_final_state &= !(1 << bit_pos);
                            }
                        }
                        // Update buffer bytes based on filtered state
                        filtered_write_buffer[1] = (filtered_final_state >> 8) as u8;
                        filtered_write_buffer[2] = filtered_final_state as u8;
                        filtered_write_buffer[3..19].fill(0);


                        // --- Optional: Read receiver's current state and merge ---
                        // This part makes it more complex. If you want the output to *combine*
                        // with the receiver's own state, you'd read it first.
                        // For simplicity, let's just *set* the state based on calculation.
                        // If merging is needed, uncomment and adapt:
                        read_buffer[0] = FEATURE_REPORT_ID;
                        if let Ok(bytes) = device.get_feature_report(&mut read_buffer) {
                            if bytes >= 3 {
                                let receiver_current_low = read_buffer[1];
                                let receiver_current_high = read_buffer[2];
                                // Merge logic here, e.g., ORing the states
                                filtered_write_buffer[1] |= receiver_current_low;
                                filtered_write_buffer[2] |= receiver_current_high;
                            }
                        }
                        // --- End Optional Merge ---

                        log::debug!(
                            "Worker: Attempting send to receiver[{}], state: {}, buffer ({} bytes): {:02X?}",
                            i,
                            filtered_final_state,
                            19, // Log the length being sent
                            &filtered_write_buffer[0..19] // Log the full slice
                        );


                        // Send the potentially filtered feature report
                        match device.send_feature_report(&filtered_write_buffer[0..REPORT_BUFFER_SIZE]) {
                            Ok(_) => {
                                log::debug!("Worker: Send to receiver[{}] successful.", i);
                                // Successfully sent. Update UI state for this receiver.
                                if let Some(shared_state) = data.receiver_states_shared.get(i) {
                                    if let Ok(mut guard) = shared_state.lock() {
                                        // Update with the state *we sent*
                                        let state_val = merge_u8_into_u16(filtered_write_buffer[1], filtered_write_buffer[2]);
                                        log::debug!("Worker: Updating receiver_states_shared[{}] from {} to {}", i, *guard, state_val);
                                        *guard = state_val;
                                    }
                                }
                            }
                            Err(e) => {
                                log::warn!("Error writing to receiver {}: {}. Attempting reopen.", i, e);
                                if let Some(shared_state) = data.receiver_states_shared.get(i) {
                                    if let Ok(mut guard) = shared_state.lock() { *guard = 0; } // Reset UI state
                                }
                                // Attempt to reopen
                                *device_opt = hidapi.open(
                                    data.receivers_config[i].vendor_id,
                                    data.receivers_config[i].product_id,
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
                        }
                    }
                    Err(e_zero) => {
                        // Handle error sending the zero state reset
                        log::warn!("Worker: Error sending zero state reset to receiver[{}]: {:?}", i, e_zero);
                        // Reset UI state, attempt reopen
                        if let Some(shared_state) = data.receiver_states_shared.get(i) {
                            if let Ok(mut guard) = shared_state.lock() { *guard = 0; }
                        }
                        log::debug!("Worker: Attempting to reopen receiver[{}] after zero-send failure...", i);
                        *device_opt = hidapi.open( data.receivers_config[i].vendor_id,
                                                   data.receivers_config[i].product_id
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
    // Send a 'zero' report to all devices on exit
    let cleanup_buffer: [u8; REPORT_BUFFER_SIZE] = {
        let mut buf = [0u8; REPORT_BUFFER_SIZE];
        buf[0] = FEATURE_REPORT_ID; // Set Report ID 4
        // All other bytes (1-18) remain 0 for the zero state
        buf
    };
    for device_opt in source_devices.iter_mut().chain(receiver_devices.iter_mut()) {
        if let Some(device) = device_opt {
            if let Err(e) = device.send_feature_report(&cleanup_buffer) {
                log::warn!("Error sending cleanup report: {}", e);
            }
        }
    }
    log::info!("Worker thread cleanup complete. Exiting.");
    // HidApi and HidDevices are dropped automatically here, closing handles.
}
