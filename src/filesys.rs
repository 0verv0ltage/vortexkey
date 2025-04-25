// vortexkey - Data compression resistant video generator.
// Copyright 2025 0verv0ltage
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! File system access.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::constants::FRAME_DIR;

/// Gets the directory where frames are stored before
/// being combined into a video file or after being
/// extracted from a video file.
/// Creates directory if it doesnt exit.
pub fn get_framebuffer_folder() -> Result<PathBuf> {
    let temp_dir = env::temp_dir();
    let frame_dir = temp_dir.join(FRAME_DIR);

    if !frame_dir.exists() {
        fs::create_dir(&frame_dir).context("Unable to crate frame directory.")?;
    }
    Ok(frame_dir)
}

/// Clears the directory specified by `get_framebuffer_folder()`
/// by deleting and recreating it.
pub fn clear_framebuffer_folder() -> Result<()> {
    let frame_dir = get_framebuffer_folder()?;
    // No existence check needed, `get_framebuffer_folder()` creates `frame_dir`
    // if it doesnt exist.
    fs::remove_dir_all(&frame_dir).context("Unable to delete frame directory.")?;
    fs::create_dir(frame_dir).context("Unable to crate frame directory.")?;
    Ok(())
}

/// Returns a file path inside the framebuffer folder used to save a frame.
///
/// # Arguments
/// * `index` - Number of the frame in the video to be created.
pub fn frame_path_combine(index: usize) -> Result<PathBuf> {
    Ok(get_framebuffer_folder()?.join(format!("combine{index:0>12}.png")))
}

/// Returns glob wildcard over all split frames in the frambuffer directory.
pub fn frame_path_wildcard_split() -> Result<PathBuf> {
    Ok(get_framebuffer_folder()?.join(Path::new("split*.png")))
}

/// Returns glob wildcard over all combine frames in the frambuffer directory.
pub fn frame_path_wildcard_combine() -> Result<PathBuf> {
    Ok(get_framebuffer_folder()?.join(Path::new("combine*.png")))
}
