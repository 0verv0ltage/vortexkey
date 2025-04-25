// vortexkey - Data compression resistant video generator.
// Copyright 2025 0verv0ltage
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! General utility functions.

use chrono::Duration;
use chrono::Local;
use std::{path::Path, time};

/// Generate a uniqe directory path based on the current ISO timestamp.
/// If path exists tries prepending increasing number until available path is found.
pub fn _generate_unique_timestamp_dir(base_dir: &str) -> String {
    let now = Local::now();
    let timestamp = now.format("%Y-%m-%dT%H:%M:%S").to_string();

    let mut counter: u64 = 1;
    let mut candidate = format!("{base_dir}{timestamp}");

    // If the base timestamp exists, prepend numbers until available path is found.
    while Path::new(&candidate).exists() {
        candidate = format!("{base_dir}{counter}_{timestamp}");
        counter += 1;
    }

    candidate
}

/// Return single hex representation of a byte slice.
pub fn bytes_to_hex_string(bytes: &[u8]) -> String {
    let mut hex_string = String::with_capacity(2 + bytes.len() * 2);
    hex_string.push_str("0x");

    for byte in bytes {
        hex_string.push_str(&format!("{byte:02x}"));
    }
    hex_string
}

/// Format a duration to human readable form.
pub fn format_duration(duration: time::Duration) -> String {
    let Ok(chrono_duration) = Duration::from_std(duration) else {
        return "Duration too large".to_string();
    };

    if chrono_duration < Duration::milliseconds(1) {
        // Microseconds (µs)
        format!("{} µs", chrono_duration.num_microseconds().unwrap_or(0))
    } else if chrono_duration < Duration::seconds(1) {
        // Milliseconds (ms)
        format!("{} ms", chrono_duration.num_milliseconds())
    } else if chrono_duration < Duration::minutes(1) {
        // Seconds (s)
        format!("{} s", chrono_duration.num_seconds())
    } else {
        // Hours:Minutes:Seconds (hh:mm:ss)
        format!(
            "{:02}:{:02}:{:02}",
            chrono_duration.num_hours(),
            chrono_duration.num_minutes() % 60,
            chrono_duration.num_seconds() % 60
        )
    }
}
