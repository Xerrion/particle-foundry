//! Explicit little-endian Rust/WGSL scalar layouts; never memcpy a Rust Vec or enum.

use crate::{
    Grid,
    contracts::{ContractError, MAX_ACTIVE_COMPONENTS},
};

/// Cell header in a WGSL storage array: `energy_j: f32, fixed_wall: u32`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuCellHeader {
    energy_j: f32,
    fixed_wall: u32,
}

impl GpuCellHeader {
    /// Validates f64-to-f32 conversion and the fixed-wall encoding.
    pub fn new(energy_j: f64, fixed_wall: u8) -> Result<Self, ContractError> {
        let energy = energy_j as f32;
        if !energy_j.is_finite() || !energy.is_finite() || fixed_wall > 1 {
            return Err(ContractError::Invalid(
                "GPU cell energy or wall flag is invalid",
            ));
        }
        Ok(Self {
            energy_j: energy,
            fixed_wall: fixed_wall as u32,
        })
    }

    /// Deterministic bytes at WGSL offsets 0 and 4; no implicit padding copy.
    pub fn to_le_bytes(self) -> [u8; 8] {
        let mut bytes = [0; 8];
        bytes[..4].copy_from_slice(&self.energy_j.to_le_bytes());
        bytes[4..].copy_from_slice(&self.fixed_wall.to_le_bytes());
        bytes
    }

    /// Rejects undefined wall encodings after a readback.
    pub fn from_le_bytes(bytes: [u8; 8]) -> Result<Self, ContractError> {
        let energy = f32::from_le_bytes(bytes[..4].try_into().expect("four-byte slice"));
        let wall = u32::from_le_bytes(bytes[4..].try_into().expect("four-byte slice"));
        Self::new(
            energy as f64,
            wall.try_into()
                .map_err(|_| ContractError::Invalid("GPU wall flag exceeds u8"))?,
        )
    }

    /// The stored passive energy marker in joules.
    pub fn energy_j(self) -> f32 {
        self.energy_j
    }

    /// True for a fixed blocked cell.
    pub fn is_wall(self) -> bool {
        self.fixed_wall == 1
    }
}

/// WGSL uniform block with 16-byte alignment and four explicit u32 slots.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GpuGridParams {
    width: u32,
    height: u32,
    active_count: u32,
    reserved: u32,
}

impl GpuGridParams {
    /// Records dimensions and the actual compact active set, never all 118 identities.
    pub fn new(grid: Grid, active_count: usize) -> Result<Self, ContractError> {
        if active_count == 0 || active_count > MAX_ACTIVE_COMPONENTS {
            return Err(ContractError::Capacity {
                requested: active_count,
                maximum: MAX_ACTIVE_COMPONENTS,
            });
        }
        Ok(Self {
            width: grid.width(),
            height: grid.height(),
            active_count: active_count as u32,
            reserved: 0,
        })
    }

    /// Packs the uniform without copying Rust padding.
    pub fn to_le_bytes(self) -> [u8; 16] {
        let mut bytes = [0; 16];
        for (slot, value) in [self.width, self.height, self.active_count, self.reserved]
            .into_iter()
            .enumerate()
        {
            bytes[slot * 4..slot * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }
}

/// Byte offset of one f32 mass in component-major GPU storage.
pub fn mass_byte_offset(
    grid: Grid,
    active_count: usize,
    slot: usize,
    cell: usize,
) -> Result<usize, ContractError> {
    if active_count == 0
        || active_count > MAX_ACTIVE_COMPONENTS
        || slot >= active_count
        || cell >= grid.cells()
    {
        return Err(ContractError::Invalid(
            "GPU component slot or cell is out of bounds",
        ));
    }
    Ok((slot * grid.cells() + cell) * std::mem::size_of::<f32>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::offset_of;

    #[test]
    fn scalar_offsets_match_wgsl() {
        assert_eq!(offset_of!(GpuCellHeader, energy_j), 0);
        assert_eq!(offset_of!(GpuCellHeader, fixed_wall), 4);
        assert_eq!(offset_of!(GpuGridParams, width), 0);
        assert_eq!(offset_of!(GpuGridParams, height), 4);
        assert_eq!(offset_of!(GpuGridParams, active_count), 8);
        assert_eq!(offset_of!(GpuGridParams, reserved), 12);
    }
}
