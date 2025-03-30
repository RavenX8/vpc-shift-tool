use clap::Parser;
use chrono::NaiveDate;
use log::{error, info, trace, warn};

pub(crate) const FEATURE_REPORT_ID_SHIFT: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReportFormat {
    pub name: &'static str,
    pub report_id: u8,
    pub total_size: usize,
    high_byte_idx: usize,
    low_byte_idx: usize,
}

impl ReportFormat {
    /// Packs the u16 state into the provided buffer according to this format's rules.
    ///
    /// It sets the report ID, places the high and low bytes of the state at the
    /// correct indices, and zeros out any remaining padding bytes up to `total_size`.
    /// Assumes the provided `buffer` is large enough to hold `total_size` bytes.
    ///
    /// # Arguments
    /// * `buffer`: A mutable byte slice, assumed to be large enough (e.g., MAX_REPORT_SIZE).
    ///           The relevant part (`0..total_size`) will be modified.
    /// * `state`: The `u16` state value to pack.
    ///
    /// # Returns
    /// A slice `&'buf [u8]` representing the packed report (`&buffer[0..self.total_size]`).
    /// Returns an empty slice if the buffer is too small.
    pub fn pack_state<'buf>(
        &self,
        buffer: &'buf mut [u8],
        state: u16,
    ) -> &'buf [u8] {
        // 1. Safety Check: Ensure buffer is large enough
        if buffer.len() < self.total_size {
            error!(
                "Buffer too small (len={}) for packing report format '{}' (size={})",
                buffer.len(),
                self.name,
                self.total_size
            );
            // Return empty slice to indicate error, calling code should handle this
            return &[];
        }

        // 2. Clear the portion of the buffer we will use (safer than assuming zeros)
        //    This handles the zero-padding requirement automatically.
        buffer[0..self.total_size].fill(0);

        // 3. Set the Report ID (Byte 0)
        buffer[0] = self.report_id;

        // 4. Pack state bytes into their defined indices
        //    Check indices against buffer length again just in case format is invalid
        if self.high_byte_idx != usize::MAX {
            if self.high_byte_idx < self.total_size { // Check index within format size
                buffer[self.high_byte_idx] = (state >> 8) as u8;
            } else { error!("High byte index {} out of bounds for format '{}' (size={})", self.high_byte_idx, self.name, self.total_size); }
        } else if (state >> 8) != 0 {
            warn!("pack_state ({}): State {} has high byte, but format doesn't support it.", self.name, state);
        }

        if self.low_byte_idx < self.total_size {
            buffer[self.low_byte_idx] = state as u8; // Low byte
        } else {
            error!("Low byte index {} out of bounds for format '{}' (size={})", self.low_byte_idx, self.name, self.total_size);
        }

        // 5. Return the slice representing the fully packed report
        &buffer[0..self.total_size]
    }

    /// Unpacks the u16 state from a received buffer slice based on this format's rules.
    ///
    /// Checks the report ID and minimum length required by the format.
    /// Extracts the high and low bytes from the specified indices and merges them.
    ///
    /// # Arguments
    /// * `received_data`: A byte slice containing the data read from the HID device
    ///                   (should include the report ID at index 0).
    ///
    /// # Returns
    /// `Some(u16)` containing the unpacked state if successful, `None` otherwise
    /// (e.g., wrong report ID, buffer too short).
    pub fn unpack_state(&self, received_data: &[u8]) -> Option<u16> {
        // 1. Basic Checks: Empty buffer or incorrect Report ID
        if received_data.is_empty() || received_data[0] != self.report_id {
            trace!(
                "unpack_state ({}): Invalid ID (expected {}, got {}) or empty buffer.",
                self.name, self.report_id, if received_data.is_empty() { "N/A".to_string() } else { received_data[0].to_string() }
            );
            return None;
        }

        // 2. Determine minimum length required based on defined indices
        //    We absolutely need the bytes up to the highest index used.
        let low_byte = if received_data.len() > self.low_byte_idx {
            received_data[self.low_byte_idx]
        } else {
            warn!("unpack_state ({}): Received data length {} too short for low byte index {}.", self.name, received_data.len(), self.low_byte_idx);
            return None;
        };

        let high_byte = if self.high_byte_idx != usize::MAX { // Does format expect a high byte?
            if received_data.len() > self.high_byte_idx { // Did we receive enough data for it?
                received_data[self.high_byte_idx]
            } else { // Expected high byte, but didn't receive it
                trace!("unpack_state ({}): Received data length {} too short for high byte index {}. Assuming 0.", self.name, received_data.len(), self.high_byte_idx);
                0
            }
        } else { // Format doesn't define a high byte
            0
        };
        // --- End Graceful Handling ---


        // 4. Merge bytes
        let state = (high_byte as u16) << 8 | (low_byte as u16);

        trace!("unpack_state ({}): Extracted state {}", self.name, state);
        Some(state)
    }
}

const FORMAT_ORIGINAL: ReportFormat = ReportFormat {
    name: "Original (Size 2)", // Add name
    report_id: FEATURE_REPORT_ID_SHIFT,
    total_size: 2,
    high_byte_idx: usize::MAX,
    low_byte_idx: 1,
};

const FORMAT_NEW: ReportFormat = ReportFormat {
    name: "NEW (Size 19)", // Add name
    report_id: FEATURE_REPORT_ID_SHIFT,
    total_size: 19,
    high_byte_idx: 1,
    low_byte_idx: 2,
};

struct FormatRule {
    // Criteria: Function that takes firmware string and returns true if it matches
    matches: fn(&str, &str) -> bool,
    // Result: The format to use if criteria matches
    format: ReportFormat,
}

const FORMAT_RULES: &[FormatRule] = &[
    // Rule 1: Check for Original format based on date
    FormatRule {
        matches: |name, fw| {
            const THRESHOLD: &str = "2024-12-26";
            let date_str = fw.split_whitespace().last().unwrap_or("");
            if date_str.len() == 8 {
                if let Ok(fw_date) = NaiveDate::parse_from_str(date_str, "%Y%m%d") {
                    if let Ok(t_date) = NaiveDate::parse_from_str(THRESHOLD, "%Y-%m-%d") {
                        return fw_date < t_date; // Return true if older
                    }
                }
            }
            false // Don't match if parsing fails or format wrong
        },
        format: FORMAT_ORIGINAL,
    },
    // Rule 2: Add more rules here if needed (e.g., for FORMAT_MIDDLE)
    // FormatRule { matches: |fw| fw.contains("SPECIAL"), format: FORMAT_MIDDLE },

    // Rule N: Default rule (matches anything if previous rules didn't)
    // This isn't strictly needed if we have a default below, but can be explicit.
    // FormatRule { matches: |_| true, format: FORMAT_NEW },
];

// --- The main function to determine the format ---
pub(crate) fn determine_report_format(name: &str, firmware: &str) -> ReportFormat {
    // Iterate through the rules
    for rule in FORMAT_RULES {
        if (rule.matches)(name, firmware) {
            trace!("Device '{}' Firmware '{}' matched rule for format '{}'", name, firmware, rule.format.name);
            return rule.format;
        }
    }

    // If no rules matched, return a default (e.g., the newest format)
    let default_format = FORMAT_NEW; // Define the default
    warn!(
        "Firmware '{}' did not match any specific rules. Defaulting to format '{}'",
        firmware, default_format.name
    );
    default_format
}

pub(crate) const MAX_REPORT_SIZE: usize = FORMAT_NEW.total_size;

/// Reads a specific bit from a u16 value.
/// `position` is 0-indexed (0-15).
pub(crate) fn read_bit(value: u16, position: u8) -> bool {
    if position > 15 {
        warn!("read_bit called with invalid position: {}", position);
        return false;
    }
    (value & (1 << position)) != 0
}


/// Checks if a device firmware string is supported.
/// TODO: Implement actual firmware checking logic if needed.
pub(crate) fn is_supported(firmware_string: String) -> bool {
    // Currently allows all devices.
    let args = crate::Args::parse(); // Need to handle args properly
    if args.skip_firmware { return true; }

    // Example fixed list check:
    // let supported_firmware = [
    //     // "VIRPIL Controls 20220720",
    //     // "VIRPIL Controls 20230328",
    //     // "VIRPIL Controls 20240323",
    //     "VIRPIL Controls 20241226",
    // ];

    if firmware_string.is_empty() || firmware_string == "Unknown Firmware" {
        warn!("Device has missing or unknown firmware string.");
        // Decide if these should be allowed or not. Allowing for now.
    }
    true
}
