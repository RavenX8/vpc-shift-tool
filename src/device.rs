use hidapi::{DeviceInfo, HidApi, HidError};
use log::{error, info, warn}; // Use log crate
use serde::{Deserialize, Serialize};
use std::rc::Rc; // Keep Rc for potential sharing within UI if needed

// Represents a discovered VPC device
#[derive(Debug, PartialEq, PartialOrd, Ord, Eq, Hash, Clone)]
pub struct VpcDevice {
    pub full_name: String, // Combined identifier
    pub name: Rc<String>,  // Product String
    pub firmware: Rc<String>, // Manufacturer String (often firmware version)
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial_number: String,
    pub usage: u16, // HID usage page/id (less commonly needed for opening)
    pub active: bool, // Is the worker thread currently connected?
}

impl Default for VpcDevice {
    fn default() -> Self {
        Self {
            full_name: String::from(""),
            name: String::from("-NO CONNECTION (Select device from list)-").into(),
            firmware: String::from("").into(),
            vendor_id: 0,
            product_id: 0,
            serial_number: String::from(""),
            usage: 0,
            active: false,
        }
    }
}

// How the device is displayed in dropdowns
impl std::fmt::Display for VpcDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.vendor_id == 0 && self.product_id == 0 {
            // Default/placeholder entry
            write!(f, "{}", self.name)
        } else {
            write!(
                f,
                "VID:{:04X} PID:{:04X} {} (SN:{} FW:{})", // More info
                self.vendor_id,
                self.product_id,
                self.name,
                if self.serial_number.is_empty() { "N/A" } else { &self.serial_number },
                if self.firmware.is_empty() { "N/A" } else { &self.firmware }
            )
        }
    }
}

// Data structure for saving selected devices in config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedDevice {
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial_number: String,
    pub state_enabled: [bool; 8], // Which shift bits are active for this device
}

impl Default for SavedDevice {
    fn default() -> Self {
        Self {
            vendor_id: 0,
            product_id: 0,
            serial_number: String::from(""),
            state_enabled: [true; 8], // Default to all enabled
        }
    }
}

/// Finds the index in the `device_list` corresponding to the saved device data.
/// Returns 0 (default "No Connection") if not found or if saved_device is invalid.
// Make this function standalone or static, not requiring &self
pub(crate) fn find_device_index_for_saved(
    device_list: &[VpcDevice], // Pass device list explicitly
    saved_device: &SavedDevice,
) -> usize {
    if saved_device.vendor_id == 0 && saved_device.product_id == 0 {
        return 0; // Point to the default "No Connection" entry
    }
    device_list
        .iter()
        .position(|d| {
            d.vendor_id == saved_device.vendor_id
                && d.product_id == saved_device.product_id
                && d.serial_number == saved_device.serial_number
        })
        .unwrap_or(0) // Default to index 0 ("No Connection") if not found
}


// --- Device Management Functions ---

// Now part of ShiftTool impl block
impl crate::ShiftTool {
    /// Refreshes the internal list of available HID devices.
    pub(crate) fn refresh_devices(&mut self) {
        info!("Refreshing device list...");
        match HidApi::new() {
            Ok(hidapi) => {
                let mut current_devices: Vec<VpcDevice> = Vec::new();
                // Keep track of seen devices to avoid duplicates
                // Use a HashSet for efficient checking
                use std::collections::HashSet;
                let mut seen_devices = HashSet::new();

                for device_info in hidapi.device_list() {
                    // Filter for specific vendor if desired
                    if device_info.vendor_id() == crate::hid_worker::VENDOR_ID_FILTER {
                        if let Some(vpc_device) =
                            create_vpc_device_from_info(device_info)
                        {
                            // Create a unique key for the device
                            let device_key = (
                                vpc_device.vendor_id,
                                vpc_device.product_id,
                                vpc_device.serial_number.clone(),
                            );

                            // Check if we've already added this unique device
                            if seen_devices.insert(device_key) {
                                // If insert returns true, it's a new device
                                if crate::util::is_supported(
                                    vpc_device.firmware.to_string(),
                                ) {
                                    info!("Found supported device: {}", vpc_device);
                                    current_devices.push(vpc_device);
                                } else {
                                    warn!(
                                        "Found unsupported device (firmware?): {}",
                                        vpc_device
                                    );
                                    // Optionally add unsupported devices too, just filter later?
                                    // current_devices.push(vpc_device);
                                }
                            } else {
                                // Device already seen (duplicate entry from hidapi)
                                log::trace!("Skipping duplicate device entry: {}", vpc_device);
                            }
                        }
                    }
                }

                // Sort devices (e.g., by name)
                current_devices.sort_by(|a, b| a.name.cmp(&b.name));

                // Add the default "no connection" entry *after* sorting real devices
                current_devices.insert(0, VpcDevice::default());


                // Update the app's device list
                self.device_list = current_devices;
                info!(
                    "Device list refresh complete. Found {} unique devices.",
                    self.device_list.len() - 1 // Exclude default entry
                );

                // Validate selected devices against the new, deduplicated list
                self.validate_selected_devices();

            }
            Err(e) => {
                error!("Failed to create HidApi for device refresh: {}", e);
            }
        }
    }

    /// Finds the index in the `device_list` corresponding to the saved receiver config.
    pub(crate) fn find_receiver_device_index(&self, receiver_config_index: usize) -> usize {
        self.find_device_index_for_saved(
            &self.config.data.receivers[receiver_config_index]
        )
    }

    /// Finds the index in the `device_list` corresponding to the saved source config.
    pub(crate) fn find_source_device_index(&self, source_config_index: usize) -> usize {
        self.find_device_index_for_saved(
            &self.config.data.sources[source_config_index]
        )
    }

    /// Generic helper to find a device index based on SavedDevice data.
    fn find_device_index_for_saved(&self, saved_device: &SavedDevice) -> usize {
        if saved_device.vendor_id == 0 && saved_device.product_id == 0 {
            return 0; // Point to the default "No Connection" entry
        }
        self.device_list
            .iter()
            .position(|d| {
                d.vendor_id == saved_device.vendor_id
                    && d.product_id == saved_device.product_id
                    && d.serial_number == saved_device.serial_number
            })
            .unwrap_or(0) // Default to index 0 ("No Connection") if not found
    }

    /// Checks if saved source/receiver devices still exist in the refreshed list.
    /// Resets the config entry to default if the device is gone.
    fn validate_selected_devices(&mut self) {
        let mut changed = false;
        for i in 0..self.config.data.sources.len() {
            let idx = self.find_source_device_index(i);
            if idx == 0 && (self.config.data.sources[i].vendor_id != 0 || self.config.data.sources[i].product_id != 0) {
                warn!("Previously selected source device {} not found after refresh. Resetting.", i + 1);
                self.config.data.sources[i] = SavedDevice::default();
                changed = true;
            }
        }
        for i in 0..self.config.data.receivers.len() {
            let idx = self.find_receiver_device_index(i);
            if idx == 0 && (self.config.data.receivers[i].vendor_id != 0 || self.config.data.receivers[i].product_id != 0) {
                warn!("Previously selected receiver device {} not found after refresh. Resetting.", i + 1);
                self.config.data.receivers[i] = SavedDevice::default();
                changed = true;
            }
        }
        if changed {
            // Optionally save the config immediately after validation changes
            // if let Err(e) = self.config.save() {
            //     error!("Failed to save config after device validation: {}", e);
            // }
        }
    }
}


/// Creates a VpcDevice from HidApi's DeviceInfo.
fn create_vpc_device_from_info(device_info: &DeviceInfo) -> Option<VpcDevice> {
    // ... (same as before)
    let vendor_id = device_info.vendor_id();
    let product_id = device_info.product_id();
    let name = device_info
        .product_string()
        .unwrap_or("Unknown Product")
        .to_string();
    let firmware = device_info
        .manufacturer_string()
        .unwrap_or("Unknown Firmware")
        .to_string();
    let serial_number = device_info.serial_number().unwrap_or("").to_string();
    let usage = device_info.usage();

    if vendor_id == 0 || product_id == 0 || name == "Unknown Product" {
        return None;
    }

    let full_name = format!(
        "{:04X}:{:04X}:{}",
        vendor_id,
        product_id,
        if serial_number.is_empty() { "no_sn" } else { &serial_number }
    );

    Some(VpcDevice {
        full_name,
        name: name.into(),
        firmware: firmware.into(),
        vendor_id,
        product_id,
        serial_number,
        usage,
        active: false,
    })
}
