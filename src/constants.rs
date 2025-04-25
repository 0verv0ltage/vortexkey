// vortexkey - Data compression resistant video generator.
// Copyright 2025 0verv0ltage
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Global constants.

// === Configuration Constants ===
#[allow(clippy::doc_markdown)]
/// Folder where intermediate frames are stored
/// before being stiched into a video.
/// This folder is created in the temp directory
/// determined using `env::temp_dir()`.
/// Default: "vortexkey_framebuffer"
pub const FRAME_DIR: &str = "vortexkey_framebuffer";

/// Path to ffmpeg executable.
/// Default: "/bin/ffmpeg"
pub const FFMPEG_EXCUTABLE_PATH: &str = "/bin/ffmpeg";

#[allow(clippy::doc_markdown)]
/// H.264 ConstantRateFactor  
/// Allowed values: 0-51  
/// 0 -> Lossless, 23 -> ffmpeg default, 51 -> worst possible  
/// Subjectively sane range is 17–28
/// Default: 20
pub const H264_CRF: u32 = 20;

/// H.264 Preset  
/// Controls encoder speed to compression ratio
/// Slower -> Smaler file size.  
/// Default: faster  
/// Valid:
/// - ultrafast
/// - superfast
/// - veryfast
/// - faster
/// - fast
/// - medium
/// - slow
/// - slower
/// - veryslow  
pub const H264_PRESET: &str = "veryfast";

/// How many fully blank buffer frames to add before the main data stream.  
/// Default: 3
pub const PREBUFFER_FRAMES: usize = 3;

/// How many fully blank buffer frames to add after the main data stream.  
/// Default: 3
pub const POSTBUFFER_FRAMES: usize = 3;

/// When reprocessing the frames extracted from a video file
/// they are scaled down to `downsample_scaler * data_resolution` first
/// and then averaged in code.  
/// Default: 2
pub const DOWNSAMPLE_SCALER: u32 = 2;

/// What colorspace to encode video as.
/// bt709 is reccomended for Youtube.  
/// Default: "bt709"
pub const COLORSPACE: &str = "bt709";

/// Video encoding color range.  
/// Default: "tv"
pub const COLOR_RANGE: &str = "tv";

// === Fixed Constants ===
// DO NOT CHANGE THESE

/// Common display resolutions
/// Includes all supported by youtube
pub mod resolutions {
    /// SD (240p)
    pub const SD_240: [u32; 2] = [426, 240];
    /// SD (360p)
    pub const SD_360: [u32; 2] = [640, 360];
    /// SD (480p)
    pub const SD_480: [u32; 2] = [854, 480];
    /// HD (720p)
    pub const HD_720: [u32; 2] = [1280, 720];
    /// Full HD (2K, 1080p)
    pub const HD_1080: [u32; 2] = [1920, 1080];
    /// Quad HD (2.5K, 1440p)
    pub const QHD_1440: [u32; 2] = [2560, 1440];
    /// Ultra HD (4K, 2160p)
    pub const UHD_4K: [u32; 2] = [3840, 2160];
    /// Ultra HD (8K, 4320p)
    pub const UHD_8K: [u32; 2] = [7680, 4320];
}

/// How many color channels we use: red, green, blue
pub const COLOR_CHANNELS: usize = 3;

/// Positions of the data bits for a Hamming(31,26) code.
pub const HAMMING_DATA_POSITIONS_31_26: [usize; 26] = [
    2, 4, 5, 6, 8, 9, 10, 11, 12, 13, 14, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29,
    30,
];

/// Positions of the parity bits for a Hamming(31,26) code.
pub const HAMMING_PARITY_POSITIONS_31_26: [u32; 5] = [1, 2, 4, 8, 16];

/// How many data bits in a Hamming(31,26) encoding
pub const HAMMING_DATA_BITS_31_26: usize = 26;

/// Masks of the lower 26 bits.
pub const BIT_MASK_26: u32 = (1 << 26) - 1;

/// Masks of the lower 31 bits.
pub const BIT_MASK_31: u32 = (1 << 31) - 1;

/// How many bytes in a data chunk for hamming encoding.
/// lcm(8 bits/byte,26 bits) / 8bits/byte = 13
pub const HAMMING_CHUNK_BYTES_31_26: usize = 13;

/// How many bytes in a chunk when parity is added.
pub const HAMMING_CHUNK_BYTES_TOAL_31_26: usize = 16;

/// Bytes in a u32
pub const BYTES_U32: usize = (u32::BITS / u8::BITS) as usize;
