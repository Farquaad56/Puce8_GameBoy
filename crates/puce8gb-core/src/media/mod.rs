//! Cartridge mapper (decisions A_06/A_07): ROM-only ($00) first; bank switching, RAM and
//! RTC arrive with later tasks. The ROM image itself is owned by the Bus (decision A_02).

pub mod header;

/// Mapper state for the loaded cartridge.
#[derive(Debug, Default)]
pub struct Media;
