//! Cartridge mapper (decisions A_06/A_07): ROM-only ($00) first; bank switching, RAM and
//! RTC arrive with later tasks. The ROM image itself is owned by the Bus (decision A_02).

pub mod header;
pub mod mapper;
pub mod mbc1;
pub mod mbc2;
pub mod mbc3;
pub mod mbc5;

/// Mapper state for the loaded cartridge. The mapper itself is built by later tasks
/// from the type code at $0147 (decision A_06); see `mapper::Mapper`.
#[derive(Debug, Default)]
pub struct Media;

/// Error returned by the battery RAM API of the mappers (E05_06).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryError {
    /// The cartridge has no persistent data to load into.
    NoBatteryData,
    /// The payload length differs from the expected one.
    SizeMismatch { expected: usize, actual: usize },
}

/// Copy `data` into `ram` when the sizes match (E05_06); an empty `ram` means no persistent RAM.
pub(crate) fn load_ram_payload(ram: &mut [u8], data: &[u8]) -> Result<(), BatteryError> {
    if ram.is_empty() {
        return Err(BatteryError::NoBatteryData);
    }
    if data.len() != ram.len() {
        return Err(BatteryError::SizeMismatch {
            expected: ram.len(),
            actual: data.len(),
        });
    }
    ram.copy_from_slice(data);
    Ok(())
}
