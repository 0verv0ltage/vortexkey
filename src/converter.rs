// vortexkey - Data compression resistant video generator.
// Copyright 2025 0verv0ltage
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Tools to encode and decode data from and into bitmap images.

use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
};

use anyhow::{Context, Result, bail};
use glob::glob;
use image::{GenericImageView, ImageBuffer, Pixel, RgbImage};
use sha2::{Digest, Sha256};

use crate::{
    constants::{
        COLOR_CHANNELS, COLOR_RANGE, COLORSPACE, DOWNSAMPLE_SCALER, FFMPEG_EXCUTABLE_PATH,
        H264_CRF, H264_PRESET, HAMMING_CHUNK_BYTES_31_26, HAMMING_CHUNK_BYTES_TOAL_31_26,
        POSTBUFFER_FRAMES, PREBUFFER_FRAMES,
    },
    error_correction::{HammingReport, decode_with_hamming_31_26, encode_with_hamming_31_26},
    filesys::{
        clear_framebuffer_folder, frame_path_combine, frame_path_wildcard_combine,
        frame_path_wildcard_split, get_framebuffer_folder,
    },
    utils::bytes_to_hex_string,
};

#[derive(Debug, PartialEq)]
/// Data read from a decoded videos header.
struct HeaderData {
    /// Identifying what version the used converter is.  
    /// Also used as a "magic" number to identify the beginnig of
    /// the first data frame.
    version_code: [u8; 8],
    /// Number of bytes that were encoded into the video.
    data_len: usize,
    /// SHA256 hash over the data.
    sha256_hash: [u8; 32],
}

#[derive(Debug, PartialEq)]
/// Result of error correction while decoding a file from video
pub struct FileReport {
    /// Single bit errors found and corrected.
    pub corrected_errors: u32,
    /// Double bit errors found and unable to be corrected.
    pub uncorrected_errors: u32,
    /// If the read hash matched the calculated hash over the entire file.
    pub hash_match: bool,
}

impl FileReport {
    /// Extend a hamming report into a file decoding report
    /// adding the information if file hash matched.
    ///
    /// # Arguments
    /// * `base_report` - Hamming report to extend
    /// * `hash_match` - If the calculated file hash matched the expected one.
    pub fn from_hamming_report(base_report: &HammingReport, hash_match: bool) -> Self {
        FileReport {
            corrected_errors: base_report.corrected_errors,
            uncorrected_errors: base_report.uncorrected_errors,
            hash_match,
        }
    }
}

#[derive(Debug, Clone, Copy)]
/// Used to convert arbitrary data to video.
/// Manages methods and parameters for that purpose.
pub struct Converter {
    /// How many bits of data should be encoded in the red color channel for each data unit.
    red_bits: u32,
    /// Mask over the in `red_bits` defined number of bits: `(1 << red_bits) - 1`
    red_mask: u32,
    /// How many bits of data should be encoded in the green color channel for each data unit.
    green_bits: u32,
    /// Mask over the in `green_bits` defined number of bits: `(1 << green_bits) - 1`
    green_mask: u32,
    /// How many bits of data should be encoded in the blue color channel for each data unit.
    blue_bits: u32,
    /// Mask over the in `blue_bits` defined number of bits: `(1 << blue_bits) - 1`
    blue_mask: u32,
    /// Total numer of bits encoded in each data unit:
    /// `red_bits + green_bits + blue_bits`
    total_bits: u32,
    /// Mask over the in `total_bits` defined number of bits: `(1 << total_bits) - 1`
    total_mask: u32,
    /// How many data frames per second should be encoded in the output video.
    data_fps: u32,
    /// Framerate the output video will be encoded as.  
    /// This is distinct from the framerate at wich dataframes are encoded.
    /// The `data_fps` can not exceed the video framerate set here.
    video_fps: u32,
    /// Height of of the output video in pixels.
    frame_height: u32,
    /// Width of of the output video in pixels.
    frame_width: u32,
    /// How many data units each frame should contain verticaly.
    data_height: u32,
    /// How many data units each frame should contain horizontaly.
    data_width: u32,
    /// How many data units a frame should contain in total: `data_height * data_width`
    frame_data_unit_count: usize,
    /// How many bytes will be encoded in each data frame:
    /// `(frame_data_unit_count * total_bits) / u8::MAX`
    frame_data_byte_count: usize,
}

impl Converter {
    /// Identifying what version this converter is.  
    /// Also used as a "magic" number to identify the beginnig of
    /// the first data frame.
    /// Encoded into the first frame of the output video.
    const VERSION_CODE: [u8; 8] = [68, 65, 67, 79, 0, 255, 0, 1];

    /// Lowest `data_fps` value allowed.
    const MIN_FPS: u32 = 1;

    /// Lenght in bytes of the header that will be endcoded into the first frame.
    const HEADER_LEN: usize = 48;

    /// Generates a new Converter.
    ///
    /// * `color_bits` - How many bits should be encoded in each color channel. Order: RGB
    /// * `data_fps` - How many data frames per second should be encoded in the output video.
    /// * `video_fps` - Final framerate of the output video. Must be larger or equal to and and multiple of `data_fps`.
    /// * `frame_dimensions` - (Width, Height) Resolution of the final output video. Must be multiples of the `data_dimensions`.
    /// * `data_dimensions` - (Width, Height) How many data units each frame should contain.
    pub fn new(
        color_bits: [u32; COLOR_CHANNELS],
        data_fps: u32,
        video_fps: u32,
        frame_dimensions: [u32; 2],
        data_dimensions: [u32; 2],
    ) -> Result<Self> {
        if color_bits.iter().any(|&x| x > u8::BITS) {
            bail!("Color channel bit counts must be one byte or smaller.");
        }

        if !(Self::MIN_FPS..=video_fps).contains(&data_fps) {
            bail!(
                "Data fps ({}) must be between {} and video fps ({}).",
                data_fps,
                Self::MIN_FPS,
                video_fps
            );
        }

        if video_fps % data_fps != 0 {
            bail!(
                "Video fps ({}) is not whole multiple of data fps ({}).",
                video_fps,
                data_fps
            );
        }

        if frame_dimensions[0] % data_dimensions[0] != 0 {
            bail!(
                "Frame width ({}) is not whole multiple of data width ({}).",
                frame_dimensions[0],
                data_dimensions[0]
            );
        }
        if frame_dimensions[1] % data_dimensions[1] != 0 {
            bail!(
                "Frame height ({}) is not whole multiple of data height ({}).",
                frame_dimensions[1],
                data_dimensions[1]
            );
        }

        if frame_dimensions[0] < DOWNSAMPLE_SCALER * data_dimensions[0] {
            bail!(
                "Frame width ({}) can not be smaller than data width ({}) multiplied by downsample scaler ({}).",
                frame_dimensions[0],
                data_dimensions[0],
                DOWNSAMPLE_SCALER
            );
        }

        if frame_dimensions[1] < DOWNSAMPLE_SCALER * data_dimensions[1] {
            bail!(
                "Frame height ({}) can to be smaller than data height ({}) multiplied by downsample scaler ({}).",
                frame_dimensions[1],
                data_dimensions[1],
                DOWNSAMPLE_SCALER
            );
        }
        let total_bits = color_bits.iter().sum();
        let frame_data_unit_count: usize =
            data_dimensions[0] as usize * data_dimensions[1] as usize;
        let frame_data_bit_count = total_bits as usize * frame_data_unit_count;
        if frame_data_bit_count % (u8::BITS as usize) != 0 {
            bail!(
                "Frame must encode whole number of bytes. Trying to encode {} bits.",
                frame_data_bit_count
            );
        }
        let frame_data_byte_count = frame_data_bit_count / u8::BITS as usize;
        Ok(Self {
            red_bits: color_bits[0],
            red_mask: (1 << color_bits[0]) - 1,
            green_bits: color_bits[1],
            green_mask: (1 << color_bits[1]) - 1,
            blue_bits: color_bits[2],
            blue_mask: (1 << color_bits[2]) - 1,
            total_bits,
            total_mask: (1 << total_bits) - 1,
            data_fps,
            video_fps,
            data_height: data_dimensions[1],
            data_width: data_dimensions[0],
            frame_height: frame_dimensions[1],
            frame_width: frame_dimensions[0],
            frame_data_unit_count,
            frame_data_byte_count,
        })
    }

    /// Take a slice of bytes and encode it into a bitmap image.
    /// The lenght of the supplied data should be equivalent to
    /// the amount of bytes than can be encoded into each frame (`frame_data_byte_count`).
    ///
    /// * `data` - Arbitrary bytes to encode into frame.
    fn data_to_frame(&self, data: &[u8]) -> Vec<u8> {
        assert_eq!(data.len(), self.frame_data_byte_count);
        let mut encoded_data_units =
            Vec::with_capacity(self.frame_data_unit_count * COLOR_CHANNELS);
        let mut bit_buffer: u32 = 0;
        let mut bit_count: u32 = 0;

        for &data_byte in data {
            // Move new byte into bit_buffer
            bit_buffer = (bit_buffer << u8::BITS) | data_byte as u32;
            bit_count += u8::BITS;

            // Extract data units until not enough bits left.
            while bit_count >= self.total_bits {
                let data_unit_bits: u32 =
                    (bit_buffer >> (bit_count - self.total_bits)) & self.total_mask;
                bit_count -= self.total_bits;

                #[allow(clippy::cast_possible_truncation)]
                let mut red =
                    ((data_unit_bits >> (self.green_bits + self.blue_bits)) & self.red_mask) as u8;
                red <<= u8::BITS - self.red_bits;
                if self.red_bits < 8 {
                    red |= 1 << (u8::BITS - self.red_bits - 1);
                }

                #[allow(clippy::cast_possible_truncation)]
                let mut green = ((data_unit_bits >> (self.blue_bits)) & self.green_mask) as u8;
                green <<= u8::BITS - self.green_bits;
                if self.green_bits < 8 {
                    green |= 1 << (u8::BITS - self.green_bits - 1);
                }

                #[allow(clippy::cast_possible_truncation)]
                let mut blue = (data_unit_bits & self.blue_mask) as u8;
                blue <<= u8::BITS - self.blue_bits;
                if self.blue_bits < 8 {
                    blue |= 1 << (u8::BITS - self.blue_bits - 1);
                }

                encoded_data_units.push(red);
                encoded_data_units.push(green);
                encoded_data_units.push(blue);
            }
        }
        assert_eq!(
            encoded_data_units.len(),
            self.frame_data_unit_count * COLOR_CHANNELS
        );
        encoded_data_units
    }

    /// Takes a bitmap image where each pixel represents a data unit and decodes the data contained in it.
    /// The image should be of dimensions (`data_width`, `data_height`) and contain the correct number of bytes.
    ///
    /// * `frame_data_units` - Image to decode data from.
    fn frame_to_data(&self, frame_data_units: &[u8]) -> Vec<u8> {
        assert_eq!(
            frame_data_units.len(),
            self.frame_data_unit_count * COLOR_CHANNELS
        );

        let mut decoded_bytes = Vec::with_capacity(self.frame_data_byte_count);
        let mut bit_buffer: u32 = 0;
        let mut bit_count: u32 = 0;

        // Each data unit is encoded as a byte triplett.
        for data_unit in frame_data_units.chunks_exact(COLOR_CHANNELS) {
            let red: u32 = (data_unit[0] >> (u8::BITS - self.red_bits)) as u32;
            let green: u32 = (data_unit[1] >> (u8::BITS - self.green_bits)) as u32;
            let blue: u32 = (data_unit[2] >> (u8::BITS - self.blue_bits)) as u32;
            let data_unit_bits =
                blue | (green << self.blue_bits) | (red << (self.blue_bits + self.green_bits));

            bit_buffer = (bit_buffer << self.total_bits) | data_unit_bits;
            bit_count += self.total_bits;

            while bit_count >= u8::BITS {
                #[allow(clippy::cast_possible_truncation)]
                let byte: u8 = (bit_buffer >> (bit_count - u8::BITS)) as u8;
                decoded_bytes.push(byte);
                bit_count -= u8::BITS;
            }
        }
        assert_eq!(decoded_bytes.len(), self.frame_data_byte_count);
        decoded_bytes
    }

    /// Generates a redundant header for data frames with the following structure:
    ///
    /// - Bytes 0-7:    `VERSION_CODE`
    /// - Bytes 8-15:    Data length in bytes (little-endian)
    /// - Bytes 16-47:   SHA256 hash of the data
    ///
    /// The header is triplicated for redundancy.
    ///
    /// # Arguments
    /// * `data` - The data to generate a header for
    fn data_block_header(data: &[u8]) -> [u8; Self::HEADER_LEN * 3] {
        // Create and populate the single header
        let mut header = [0u8; Self::HEADER_LEN];

        header[0..8].copy_from_slice(&Self::VERSION_CODE);
        header[8..16].copy_from_slice(&(data.len() as u64).to_le_bytes());
        let data_hash = Sha256::digest(data);
        header[16..48].copy_from_slice(&data_hash);

        // Triplicate the header for redundancy
        std::array::from_fn(|i| header[i % Self::HEADER_LEN])
    }

    /// Takes in a triple redundant header generated by `data_block_header`
    /// Converts it into a single header by majority vote of the three copies
    /// and decodes the contents.
    ///
    /// # Arguments
    /// * `header` - Triple redundant header bytes read from file
    fn read_data_header(header: &[u8; Self::HEADER_LEN * 3]) -> Result<HeaderData> {
        // Split into the three redundant headers
        let (part1, rest) = header.split_at(Self::HEADER_LEN);
        let (part2, part3) = rest.split_at(Self::HEADER_LEN);

        // Perform majority vote over three redundant copies.
        let mut majority: [u8; Self::HEADER_LEN] = [0; Self::HEADER_LEN];
        for i in 0..Self::HEADER_LEN {
            majority[i] = (part1[i] & part2[i]) | (part2[i] & part3[i]) | (part1[i] & part3[i]);
        }

        let version_code: [u8; 8] = majority[0..8].try_into()?;
        let data_len: usize = u64::from_le_bytes(majority[8..16].try_into()?)
            .try_into()
            .context("Read data lenght wont fit into pointer type.")?;
        let sha256_hash: [u8; 32] = majority[16..48].try_into()?;
        Ok(HeaderData {
            version_code,
            data_len,
            sha256_hash,
        })
    }

    /// Saves a frame at the specified location where all encoded bytes are zero.
    ///
    /// # Arguments
    /// * `frame_path` - Path where the frame should be saved.
    fn save_buffer_frame(&self, path: &Path) -> Result<()> {
        self.save_data_frame(&vec![0; self.frame_data_byte_count], path)?;
        Ok(())
    }

    /// Helper function to save an data frame
    fn save_data_frame(&self, frame_data: &[u8], path: &Path) -> Result<()> {
        let received_data_len = frame_data.len();
        if received_data_len > self.frame_data_byte_count {
            bail!(
                "Frame data supplied ({} bytes) is longer than expected ({} bytes).",
                received_data_len,
                self.frame_data_byte_count
            );
        }
        let img_data = if received_data_len < self.frame_data_byte_count {
            let mut frame_buffer: Vec<u8> = frame_data.to_vec();
            frame_buffer.resize(self.frame_data_byte_count, 0);
            self.data_to_frame(&frame_buffer)
        } else {
            self.data_to_frame(frame_data)
        };

        let img_buffer: RgbImage =
            ImageBuffer::from_raw(self.data_width, self.data_height, img_data)
                .context("Unable to create image buffer from frame data")?;
        img_buffer
            .save(path)
            .context("Unable to save frame as PNG")?;
        Ok(())
    }

    /// Read a file at the supplied path and encodes its contents it into as many frames as needed.
    /// Also prepends a header generated using `data_block_header` before generating.
    /// Saves all generated frames in the directory specified using `constants::FRAME_DIR_PATH`.
    ///
    /// # Arguments
    /// * `path` - Path where the file to read is located.
    pub fn deconstruct_file(&self, path: &Path) -> Result<()> {
        // This whole process could be optimized to not require loading the entire file into memory.
        // I didnt.
        let mut file_data = fs::read(path).context("Unable to read source file")?;
        let header = Self::data_block_header(&file_data).to_vec();

        clear_framebuffer_folder()?;

        // Generating prebuffer frames
        for i in 0..PREBUFFER_FRAMES {
            self.save_buffer_frame(&frame_path_combine(i)?)?;
        }

        // Pad with zero to whole number of hamming chunks to allow error correction.
        file_data.resize(
            file_data.len().div_ceil(HAMMING_CHUNK_BYTES_31_26) * HAMMING_CHUNK_BYTES_31_26,
            0,
        );

        println!("Encoding {:?} bytes to video.", file_data.len());

        let mut file_data_with_correction = encode_with_hamming_31_26(&file_data)?;
        file_data_with_correction.splice(0..0, header);

        // Generating regular data frames
        for (frame_index, frame_data) in file_data_with_correction
            .chunks(self.frame_data_byte_count)
            .enumerate()
        {
            self.save_data_frame(
                frame_data,
                &frame_path_combine(frame_index + PREBUFFER_FRAMES)?,
            )?;
        }

        let last_frame_index = file_data_with_correction
            .chunks(self.frame_data_byte_count)
            .count();
        let postbuffer_index_start = PREBUFFER_FRAMES + last_frame_index;
        // Generating postbuffer frames
        for i in postbuffer_index_start..postbuffer_index_start + POSTBUFFER_FRAMES {
            self.save_buffer_frame(&frame_path_combine(i)?)?;
        }
        Ok(())
    }

    /// Reads in a png image at `downsample_scaler` times the final data resolution
    /// Averages `downsample_scaler * downsample_scaler` pixel blocks and returns the data as Vec<u8>.
    ///
    /// # Arguments
    /// * `path` - Path where the frame to read is located.
    fn average_blocks<P: AsRef<Path>>(&self, path: P) -> Result<Vec<u8>> {
        let img = image::open(path).context("Failed to open image")?;
        let (width, height) = img.dimensions();

        if width != self.data_width * DOWNSAMPLE_SCALER {
            bail!(
                "Read image width ({}) is incorrect. Expected data_width * DOWNSAMPLE_SCALER ({}*{}={})",
                width,
                self.data_width,
                DOWNSAMPLE_SCALER,
                self.data_width * DOWNSAMPLE_SCALER
            );
        }

        if height != self.data_height * DOWNSAMPLE_SCALER {
            bail!(
                "Read image height ({}) is incorrect. Expected data_height * DOWNSAMPLE_SCALER ({}*{}={})",
                height,
                self.data_height,
                DOWNSAMPLE_SCALER,
                self.data_height * DOWNSAMPLE_SCALER
            );
        }

        let mut output = Vec::with_capacity((self.data_width * self.data_height * 3) as usize);

        for by in 0..self.data_height {
            for bx in 0..self.data_width {
                let mut r_sum = 0u32;
                let mut g_sum = 0u32;
                let mut b_sum = 0u32;

                for y in 0..DOWNSAMPLE_SCALER {
                    for x in 0..DOWNSAMPLE_SCALER {
                        let px = img
                            .get_pixel(bx * DOWNSAMPLE_SCALER + x, by * DOWNSAMPLE_SCALER + y)
                            .to_rgb();
                        r_sum += px[0] as u32;
                        g_sum += px[1] as u32;
                        b_sum += px[2] as u32;
                    }
                }
                let block_size = DOWNSAMPLE_SCALER * DOWNSAMPLE_SCALER;
                #[allow(clippy::cast_possible_truncation)]
                output.push((r_sum / block_size) as u8);
                #[allow(clippy::cast_possible_truncation)]
                output.push((g_sum / block_size) as u8);
                #[allow(clippy::cast_possible_truncation)]
                output.push((b_sum / block_size) as u8);
            }
        }
        Ok(output)
    }

    /// Take all frames saved in `constants::FRAME_DIR_PATH` and decode them
    /// Combining the extracted data back into a single file.
    ///
    /// # Arguments
    /// * `path` - Path where the file will be stored.
    /// * `overwrite` - If the output file should be overwritten if it exists.
    pub fn reconstruct_file<P: AsRef<Path>>(&self, path: P, overwrite: bool) -> Result<FileReport> {
        let mut read_from_video: Vec<u8> = Vec::with_capacity(self.frame_data_byte_count);
        let mut checked_header: HeaderData = HeaderData {
            version_code: [0; 8],
            data_len: 0,
            sha256_hash: [0; 32],
        };

        let mut found_header_frame = false;
        for frame_path in glob(&frame_path_wildcard_split()?.to_string_lossy())? {
            let mut img_content = self.frame_to_data(&self.average_blocks(frame_path?)?);
            debug_assert_eq!(img_content.len(), self.frame_data_byte_count);
            if found_header_frame {
                read_from_video.append(&mut img_content);
                continue;
            }
            // Read three redundant header copies.
            let header: [u8; Self::HEADER_LEN * 3] =
                img_content[0..Self::HEADER_LEN * 3].try_into()?;
            // If the header bytes are all zero we are still on a prebuffer frame.
            if header.iter().any(|&x| x != 0) {
                found_header_frame = true;
                checked_header =
                    Self::read_data_header(&header).context("Unable to decode header.")?;
                // Dont include header in read_data.
                read_from_video.append(&mut img_content[Self::HEADER_LEN * 3..].to_vec());
            }
        }

        println!("Read {:?} bytes from video.", read_from_video.len());

        // Pad with zero to whole number of hamming chunks to allow error correction.
        read_from_video.resize(
            read_from_video
                .len()
                .div_ceil(HAMMING_CHUNK_BYTES_TOAL_31_26)
                * HAMMING_CHUNK_BYTES_TOAL_31_26,
            0,
        );
        let (mut corrected_data, report) = decode_with_hamming_31_26(&read_from_video)?;

        if checked_header.version_code != Self::VERSION_CODE {
            bail!("Unable to find correct VERSION_CODE. First data frame missing or corrupted.");
        }

        if checked_header.data_len == 0 {
            bail!("Expected size read as invalid value zero.");
        }

        if checked_header.data_len > corrected_data.len() {
            bail!(
                "Read less data ({} bytes) than expected file size ({} bytes).",
                corrected_data.len(),
                checked_header.data_len
            );
        }

        // Resize to expected size.
        corrected_data.resize(checked_header.data_len, 0);

        println!("Writing {:?} bytes to file.", corrected_data.len());

        if !overwrite & path.as_ref().exists() {
            bail!("File at file output path exists and overwrite is not enabled.");
        }

        let computed_hash: [u8; 32] = Sha256::digest(&corrected_data).into();
        fs::write(path, corrected_data).context("Unable to write output file.")?;

        let report =
            FileReport::from_hamming_report(&report, computed_hash == checked_header.sha256_hash);
        if !report.hash_match {
            eprintln!(
                "Reconstructed file hash {} does not match expected hash {}.",
                bytes_to_hex_string(&computed_hash),
                bytes_to_hex_string(&checked_header.sha256_hash)
            );
        }
        Ok(report)
    }

    /// Take all frames saved in `constants::FRAME_DIR_PATH` and combine them into a video.
    /// Upscale video to `frame_height` x `frame_width` and save at specified path.
    /// Also increase framerate to `constants::VIDEO_FPS`.
    ///
    /// # Arguments
    /// * `output_file` - Path pointing to the combined video file.
    /// * `overwrite` - If the output file should be overwritten if it exists.
    pub fn combine_frames<P: AsRef<Path>>(&self, output_file: P, overwrite: bool) -> Result<()> {
        if !overwrite & output_file.as_ref().exists() {
            bail!("File at video output path exists and overwrite is not enabled.");
        }
        // Encoding parameters choosed as per youtube reccomendation:
        // https://support.google.com/youtube/answer/1722171
        // - mp4 Containter
        // - H.264
        // - Profile: High
        // - CABAC enabled
        // - bt709 colorspace
        // - Chroma subsampling: 4:2:0
        let ffmpeg_command = Command::new(FFMPEG_EXCUTABLE_PATH)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-framerate",
                &format!("{}", self.data_fps),
                "-pattern_type",
                "glob",
                "-i",
                &frame_path_wildcard_combine()?.to_string_lossy(),
                "-vf",
                // Downscaling algorithm used when splitting video back into frames.
                // Available:
                // - fast_bilinear     3 errors
                // - bilinear          6 errors
                // - bicubic           3 errors
                // - experimental      4 errors
                // - neighbor          3 errors
                // - area              2 errors
                // - bicublin          3 errors
                // - gauss             6 errors
                // - sinc (slow)       3 errors
                // - lanczos           3 errors
                // - spline (slow)     3 errors
                &format!(
                    "scale={}:{}:flags=neighbor,format=yuv420p",
                    self.frame_width, self.frame_height
                ),
                "-c:v",
                "libx264",
                "-preset",
                H264_PRESET,
                "-crf",
                &format!("{H264_CRF}"),
                "-profile:v",
                "high",
                "-colorspace:v",
                COLORSPACE,
                "-color_primaries:v",
                COLORSPACE,
                "-color_trc:v",
                COLORSPACE,
                "-color_range:v",
                COLOR_RANGE,
                "-r",
                &format!("{}", self.video_fps),
                "-y", // Overwrite if exists
                &output_file.as_ref().to_string_lossy(),
            ])
            .stdout(Stdio::null())
            //.stderr(Stdio::null())
            .status()?;
        if !ffmpeg_command.success() {
            bail!("ffmpeg returned nonzero exit status.");
        }
        Ok(())
    }

    /// Split a video back into individual frames.
    /// Also scales down back to the data resolution.
    ///
    /// # Arguments
    /// * `input_file` - Path pointing to the video file.
    pub fn split_video<P: AsRef<Path>>(&self, input_file: P) -> Result<()> {
        clear_framebuffer_folder()?;
        let frame_dir = get_framebuffer_folder()?;
        let frame_wildcard = frame_dir.join(Path::new("split%09d.png"));
        let ffmpeg_command = Command::new(FFMPEG_EXCUTABLE_PATH)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-i",
                &input_file.as_ref().to_string_lossy(),
                "-vf",
                &format!(
                    "scale={}:{}:flags=neighbor",
                    self.data_width * DOWNSAMPLE_SCALER,
                    self.data_height * DOWNSAMPLE_SCALER,
                ),
                "-r",
                &format!("{}", self.data_fps),
                &frame_wildcard.to_string_lossy(),
            ])
            .stdout(Stdio::null())
            //.stderr(Stdio::null())
            .status()?;
        if !ffmpeg_command.success() {
            bail!("ffmpeg returned nonzero exit status.");
        }
        Ok(())
    }
}
