// vortexkey - Data compression resistant video generator.
// Copyright 2025 0verv0ltage 
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! vortexkey - Data compression resistant video generator.
//! Encode arbitrary data as series of images and combining into video.
//! Also does the reverse.
//! NOTE: No effort has been undertaken to make this work on Windows. Probably wont. 🤷

// Youtube Recommended video bitrates for SDR uploads
// Type    Standard Frame Rate (24-30), High Frame Rate (48-60)
// 8K           80 - 160 Mbps   120 to 240 Mbps
// 2160p (4K)   35–45 Mbps      53–68 Mbps
// 1440p (2K)   16 Mbps         24 Mbps
// 1080p        8 Mbps          12 Mbps
// 720p         5 Mbps          7.5 Mbps
// 480p         2.5 Mbps        4 Mbps
// 360p         1 Mbps          1.5 Mbps

// TODO:
// - Base 64 config representaion
// - Split converter struct

#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    clippy::missing_docs_in_private_items,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::pedantic,
    clippy::redundant_clone,
    clippy::needless_pass_by_value
)]
#![allow(clippy::cast_lossless, dead_code)]

use std::time;

use anyhow::{Result, bail};
use clap::Parser;

use cli::{Args, OperatingMode};
use utils::format_duration;

mod cli;
mod constants;
mod converter;
mod error_correction;
mod filesys;
mod utils;

/// Times the execution of `code` and
/// prints out the measured time.
macro_rules! timed_block {
    ($name:expr, $code:block) => {
        println!("Starting {}", $name);
        let start = std::time::Instant::now();
        $code
        println!(
            "Finished {} after: {:?}",
            $name,
            start.elapsed()
        );
    };
}

/// Read in command line args and execute program function as requested.
fn execute_args() -> Result<()> {
    let args = Args::parse();
    let main_converter = args.to_converter_config()?;

    if !args.inputfile.exists() {
        bail!(
            "Provided input file at {:?} could not be found.",
            args.inputfile
        );
    }
    match args.mode {
        OperatingMode::Split => {
            timed_block!("frame generation", {
                main_converter.deconstruct_file(&args.inputfile)?;
            });
            Ok(())
        }
        OperatingMode::DataToVideo => {
            timed_block!("frame generation", {
                main_converter.deconstruct_file(&args.inputfile)?;
            });

            timed_block!("frame combination", {
                main_converter.combine_frames(&args.outputfile, args.overwrite)?;
            });
            Ok(())
        }
        OperatingMode::VideoToData => {
            timed_block!("video splitting", {
                main_converter.split_video(&args.inputfile)?;
            });
            println!("Starting .");
            let start_split_video = time::Instant::now();

            println!(
                "Finished video splitting after: {}",
                format_duration(start_split_video.elapsed())
            );

            println!("Starting file reconstruction.");
            let start_file_reconstruction = time::Instant::now();
            let report = main_converter.reconstruct_file(&args.outputfile, args.overwrite)?;
            println!(
                "Errors during file reconstruction: Corrected: {}  Uncorrectable: {}",
                report.corrected_errors, report.uncorrected_errors
            );
            println!(
                "Finished file reconstruction after: {}",
                format_duration(start_file_reconstruction.elapsed())
            );
            Ok(())
        }
    }
}

fn main() -> Result<()> {
    let main_start = time::Instant::now();

    execute_args()?;

    println!(
        "Total execution time: {}",
        format_duration(main_start.elapsed())
    );
    Ok(())
}
