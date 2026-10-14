use super::*;

/// Unmapped candidate storage owned by the sequential scene transaction.
/// Every consumed field is cleared or overwritten by its stage on each attempt.
/// Queue ordering protects reuse after cancellation of submitted work. Readback
/// staging remains attempt-local because it can still have a pending map.
pub(super) struct CandidateBuffers {
    pub transport: GpuTransportCandidate,
    pub momentum: GpuMomentumCandidate,
    pub density: GpuDerivedDensityCandidate,
    pub viscosity: Option<GpuViscosityCandidate>,
    pub gravity: GpuGravityCandidate,
}

impl CandidateBuffers {
    pub fn new(device: &wgpu::Device, grid: Grid, viscosity: bool) -> Result<Self, String> {
        Ok(Self {
            transport: GpuTransportCandidate::new(device, grid)?,
            momentum: GpuMomentumCandidate::new(device, grid)?,
            density: GpuDerivedDensityCandidate::new(device, grid)?,
            viscosity: if viscosity {
                Some(GpuViscosityCandidate::new(device, grid)?)
            } else {
                None
            },
            gravity: GpuGravityCandidate::new(device, grid)?,
        })
    }
}
