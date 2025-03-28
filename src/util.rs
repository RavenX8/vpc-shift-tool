use log::warn; // Use log crate

/// Reads a specific bit from a u16 value.
/// `position` is 0-indexed (0-15).
pub(crate) fn read_bit(value: u16, position: u8) -> bool {
    if position > 15 {
        warn!("read_bit called with invalid position: {}", position);
        return false;
    }
    (value & (1 << position)) != 0
}

/// Sets or clears a specific bit in a u8 value.
/// `bit_position` is 0-indexed (0-7).
/// Returns the modified u8 value.
pub(crate) fn set_bit(value: u8, bit_position: u8, bit_value: bool) -> u8 {
    if bit_position > 7 {
        warn!("set_bit called with invalid position: {}", bit_position);
        return value; // Return original value on error
    }
    if bit_value {
        value | (1 << bit_position) // Set the bit to 1
    } else {
        value & !(1 << bit_position) // Set the bit to 0
    }
}

/// Combines high and low bytes into a u16 value.
pub(crate) fn merge_u8_into_u16(high_byte: u8, low_byte: u8) -> u16 {
    (high_byte as u16) << 8 | (low_byte as u16)
}

/// Checks if a device firmware string is supported.
/// TODO: Implement actual firmware checking logic if needed.
pub(crate) fn is_supported(firmware_string: String) -> bool {
    // Currently allows all devices.
    // If you re-enable firmware checking, use the `args` or a config setting.
    // let args = crate::main::Args::parse(); // Need to handle args properly
    // if args.skip_firmware { return true; }

    // Example fixed list check:
    // let supported_firmware = [
    //     "VIRPIL Controls 20220720",
    //     "VIRPIL Controls 20230328",
    //     "VIRPIL Controls 20240323",
    // ];
    // supported_firmware.contains(&firmware_string.as_str())

    if firmware_string.is_empty() || firmware_string == "Unknown Firmware" {
        warn!("Device has missing or unknown firmware string.");
        // Decide if these should be allowed or not. Allowing for now.
    }
    true
}
