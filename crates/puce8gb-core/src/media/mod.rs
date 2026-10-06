//! Cartridge mapper (decisions A_06/A_07): ROM-only ($00) first; bank switching, RAM and
//! RTC arrive with later tasks. The ROM image itself is owned by the Bus (decision A_02).

pub mod header;
pub mod mapper;

/// Mapper state for the loaded cartridge. The mapper itself is built by later tasks
/// from the type code at $0147 (decision A_06); see `mapper::Mapper`.
#[derive(Debug, Default)]
pub struct Media;
