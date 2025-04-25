// vortexkey - Data compression resistant video generator.
// Copyright 2025 0verv0ltage
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! cli - Command line interface tooling.

use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::{Parser, ValueEnum};

use crate::{constants::resolutions, converter::Converter};

#[derive(ValueEnum, Clone, Debug, PartialEq)]
#[value(rename_all = "lower")]
/// Converter operating mode:  
/// - dtv (Data to Video)  
/// - vtd (Video to Data)
/// - split (Data to Frames)
pub enum OperatingMode {
    #[value(name = "dtv")]
    /// Encode a file to a video.
    DataToVideo,
    #[value(name = "vtd")]
    /// Decode a file from a video.
    VideoToData,
    #[value(name = "split")]
    /// Turn data into a series of frames.
    Split,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
/// Command line argument handler.
pub struct Args {
    /// Output file (video file or reconstructed data).
    pub outputfile: PathBuf,
    #[arg(short = 'i')]
    /// Input file (video file or data to convert).
    pub inputfile: PathBuf,
    #[arg(
        short = 'y',
        help = "If output file should be overwritten if it exists.",
        default_value_t = false
    )]
    /// If output file should be overwritten if it exists.
    pub overwrite: bool,
    #[arg(
        short,
        value_enum, 
        default_value_t = OperatingMode::DataToVideo,
        help = "Operating mode dtv (Data to Video), vtd (Video to Data) or split (Data to Frames)"
        )]
    /// Operating mode dtv (Data to Video), vtd (Video to Data) or split (Data to Frames)
    pub mode: OperatingMode,
    #[arg(
        short,
        default_value_t = 121,
        value_parser = clap::value_parser!(u32).range(111..=888),
        help = "Number of bits encoded in each color channel. (RGB)"
        )]
    /// Number of bits encoded in each color channel. (RGB)
    colorbits: u32,
    #[arg(
        long,
        default_value_t = 30,
        value_parser = clap::value_parser!(u32).range(1..=60),
        help = "Output video framerate."
        )]
    /// Output video framerate.
    video_fps: u32,
    #[arg(
        long,
        default_value_t = 1,
        value_parser = clap::value_parser!(u32).range(1..=60),
        help = "Data framerate."
        )]
    /// Data framerate.
    data_fps: u32,
    #[arg(
        short,
        long,
        value_parser = ["240p", "360p", "480p", "720p", "1080p", "1440p", "4k", "8k"],
        default_value = "1080p",
        help = "Output video resolution."
    )]
    /// Output video resolution.
    frame_resolution: String,
    #[arg(
        short,
        long,
        default_value_t = 10,
        value_parser = clap::value_parser!(u32).range(1..=100),
        help = "Size of data block in pixels."
        )]
    /// Size of data block in pixels.
    data_pixel_size: u32,
}

impl Args {
    /// Use command line arguments to constuct converter instance.
    pub fn to_converter_config(&self) -> Result<Converter> {
        let video_resolution = match self.frame_resolution.as_str() {
            "240p" => resolutions::SD_240,
            "360p" => resolutions::SD_360,
            "480p" => resolutions::SD_480,
            "720p" => resolutions::HD_720,
            "1080p" => resolutions::HD_1080,
            "1440p" => resolutions::QHD_1440,
            "4k" => resolutions::UHD_4K,
            "8k" => resolutions::UHD_8K,
            _ => bail!("Invalid resolution specified."),
        };
        let data_resolution = [
            video_resolution[0] / self.data_pixel_size,
            video_resolution[1] / self.data_pixel_size,
        ];
        Converter::new(
            [
                (self.colorbits / 100),
                (self.colorbits % 100) / 10,
                self.colorbits % 10,
            ],
            self.data_fps,
            self.video_fps,
            video_resolution,
            data_resolution,
        )
    }
}
