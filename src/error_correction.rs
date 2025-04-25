// vortexkey - Data compression resistant video generator.
// Copyright 2025 0verv0ltage
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Error correction

use anyhow::{Result, bail};

use crate::constants::{
    BIT_MASK_26, BIT_MASK_31, BYTES_U32, HAMMING_CHUNK_BYTES_31_26, HAMMING_CHUNK_BYTES_TOAL_31_26,
    HAMMING_DATA_BITS_31_26, HAMMING_DATA_POSITIONS_31_26, HAMMING_PARITY_POSITIONS_31_26,
};

#[derive(Debug, PartialEq, Eq)]
/// Reports if error was detected and if it could be corrected
/// when evaluating hamming code parity.
/// If more than two errors occured in a block this status becomes meaningless.
pub enum HammingStatus {
    /// No Error detected
    NoError,
    /// Single bit error found and corrected
    CorrectedSingle,
    /// Double bit error found, not correctable.
    Uncorrectable,
}

#[derive(Debug, PartialEq, Eq)]
/// Number of correctable and uncorrectable errors
/// found when decoding data with `decode_with_hamming_31_26`.
pub struct HammingReport {
    /// Single bit errors found and corrected.
    pub corrected_errors: u32,
    /// Double bit errors found and unable to be corrected.
    pub uncorrected_errors: u32,
}

/// Splits data into 26 bit chunks and calculates Hamming(31, 26) code
/// using `hamming_31_26_encode`.
/// The data must be provided as a multiple of 13 bytes, since lcm(8,26)/8.
/// This function does not provide any padding.
///
/// # Arguments
/// * `data` - The bytes to calculate parity for.
pub fn encode_with_hamming_31_26(data: &Vec<u8>) -> Result<Vec<u8>> {
    if data.len() % HAMMING_CHUNK_BYTES_31_26 != 0 {
        bail!(
            "Data length must be a multiple of {} bytes.",
            HAMMING_CHUNK_BYTES_31_26
        );
    }
    // Calculate number of total bytes in number of hamming groups
    // needed to encode `data`.
    let output_bytes = ((data.len() / HAMMING_DATA_BITS_31_26) * BYTES_U32) / u8::BITS as usize;
    let mut encoded = Vec::with_capacity(output_bytes);
    let mut bit_buffer: u64 = 0;
    let mut bit_counter: usize = 0;

    for byte in data {
        bit_buffer = (bit_buffer << u8::BITS) | (*byte as u64);
        bit_counter += u8::BITS as usize;

        while bit_counter >= HAMMING_DATA_BITS_31_26 {
            #[allow(clippy::cast_possible_truncation)]
            let data_bits: u32 =
                ((bit_buffer >> (bit_counter - HAMMING_DATA_BITS_31_26)) as u32) & BIT_MASK_26;
            bit_counter -= HAMMING_DATA_BITS_31_26;
            let code = hamming_31_26_encode(data_bits);
            encoded.extend_from_slice(&code.to_le_bytes());
        }
    }
    Ok(encoded)
}

/// Takes a data containing Hamming(31, 26) error correction
/// generated using the `encode_with_hamming_31_26` function.
/// And evaluates the code, correcting errors where possible
/// returning the original data or as close to the original data
/// as error correction permits.  
/// The number of corrected and uncorrected errors is reported
/// in a `HammingReport` struct.
///
/// # Arguments
/// * `data` - The bytes to evaluate.
pub fn decode_with_hamming_31_26(data: &[u8]) -> Result<(Vec<u8>, HammingReport)> {
    if data.len() % HAMMING_CHUNK_BYTES_TOAL_31_26 != 0 {
        bail!(
            "Data length must be a multiple of {} bytes.",
            HAMMING_CHUNK_BYTES_TOAL_31_26
        );
    }
    // Calculate number of data bytes encoded in number of hamming groups
    // encoded in `data`.
    let output_bytes = ((data.len() / BYTES_U32) * HAMMING_DATA_BITS_31_26) / u8::BITS as usize;
    let mut output: Vec<u8> = Vec::with_capacity(output_bytes);
    let mut bit_buffer: u64 = 0;
    let mut bit_counter: usize = 0;
    let mut corrected_errors = 0;
    let mut uncorrected_errors = 0;

    // Go over every group of four bytes since they
    // are the result of a single Hamming(31, 26) group.
    for chunk in data.chunks_exact(BYTES_U32) {
        let chunk_bytes: [u8; BYTES_U32] = chunk.try_into()?;
        let (data_bits, hamming_code) = hamming_31_26_decode(u32::from_le_bytes(chunk_bytes));
        // Keep track of number of errors.
        match hamming_code {
            HammingStatus::CorrectedSingle => corrected_errors += 1,
            HammingStatus::Uncorrectable => uncorrected_errors += 1,
            HammingStatus::NoError => (),
        }
        // Buffer the extracted 26 data bits.
        bit_counter += HAMMING_DATA_BITS_31_26;
        bit_buffer = (bit_buffer << HAMMING_DATA_BITS_31_26) | (data_bits as u64);

        // When more than 8 data bits are available, extract them from `bit_buffer`.
        while bit_counter >= u8::BITS as usize {
            #[allow(clippy::cast_possible_truncation)]
            let byte: u8 = (bit_buffer >> (bit_counter - u8::BITS as usize)) as u8;
            output.push(byte);
            bit_counter -= u8::BITS as usize;
        }
    }
    Ok((
        output,
        HammingReport {
            corrected_errors,
            uncorrected_errors,
        },
    ))
}

/// Takes 26 bits in and encodes them as 32 bits with a Hamming(31, 26) code
/// including an additional parity bit over all other bits.
///
/// # Arguments
/// * `data_bits` - The 26 data bits to calculate parity for.
fn hamming_31_26_encode(data_bits: u32) -> u32 {
    let data_bits = data_bits & BIT_MASK_26; // Mask to 26 bits
    let mut code_word = 0u32;

    // Place data bits into code_word
    for (i, &offset) in HAMMING_DATA_POSITIONS_31_26.iter().enumerate() {
        let bit = (data_bits >> i) & 1;
        code_word |= bit << offset;
    }

    // Compute parity bits
    for &p in &HAMMING_PARITY_POSITIONS_31_26 {
        let mut parity = 0;
        for bit in 0u32..31 {
            let code_word_pos = bit + 1;
            if (code_word_pos & p) != 0 {
                parity ^= (code_word >> bit) & 1;
            }
        }
        code_word |= parity << (p - 1);
    }

    // Add overall parity bit (bit 31)
    let overall_parity = code_word.count_ones() % 2;
    code_word | (overall_parity << 31)
}

/// Takes in 32 bits that include 26 data bits, 5 parity bits as
/// defined by a Hamming(31, 26) code and a parity bit over the entire group.
/// Evaluates these and performs error correction when possible.
/// If an error was found and if it could be corrected is reported
/// using a `HammingStatus`.
///
/// # Arguments
/// * `code_word` - The 32 bits to evaluate parity for.
fn hamming_31_26_decode(code_word: u32) -> (u32, HammingStatus) {
    let hamming_code = code_word & BIT_MASK_31;
    let original_parity = code_word.count_ones() % 2;
    let mut syndrome: u32 = 0;

    // Calculate syndrome for Hamming code
    for (i, &p) in HAMMING_PARITY_POSITIONS_31_26.iter().enumerate() {
        let mut parity = 0;
        for bit in 0u32..31 {
            let code_word_pos = bit + 1;
            if (code_word_pos & p) != 0 {
                parity ^= (hamming_code >> bit) & 1;
            }
        }
        if parity != 0 {
            syndrome |= 1 << i;
        }
    }

    // Determine error type and apply corrections
    let (corrected_hamming, status) = match (syndrome != 0, original_parity != 0) {
        // No errors detected
        (false, false) => (hamming_code, HammingStatus::NoError),

        // Single-bit error in parity bit
        (false, true) => {
            // Flip overall parity bit (bit 31 not included in hamming_code)
            (hamming_code, HammingStatus::CorrectedSingle)
        }

        // Single-bit error in Hamming code
        (true, true) => {
            // Flip the erroneous bit (syndrome is 1-based index)
            let error_pos = syndrome - 1;
            (
                hamming_code ^ (1 << error_pos),
                HammingStatus::CorrectedSingle,
            )
        }

        // Uncorrectable multi-bit error
        (true, false) => (hamming_code, HammingStatus::Uncorrectable),
    };

    // Extract data bits from corrected Hamming code
    let mut data = 0;
    for (i, &offset) in HAMMING_DATA_POSITIONS_31_26.iter().enumerate() {
        data |= ((corrected_hamming >> offset) & 1) << i;
    }

    (data, status)
}
