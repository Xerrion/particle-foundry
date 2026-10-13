//! Resident component-aware Galerkin hierarchy for the pressure comparison.

use super::*;

mod setup;

const NODE_BYTES: u64 = 32;

struct Level {
    width: u32,
    height: u32,
    groups: u32,
    uniform: wgpu::Buffer,
    nodes: wgpu::Buffer,
    rhs: wgpu::Buffer,
    first: wgpu::Buffer,
    second: wgpu::Buffer,
}

struct Stages {
    fine: wgpu::ComputePipeline,
    roots: wgpu::ComputePipeline,
    weights: wgpu::ComputePipeline,
    initialize: wgpu::ComputePipeline,
    smooth: wgpu::ComputePipeline,
    restrict: wgpu::ComputePipeline,
    prolong: wgpu::ComputePipeline,
    export: wgpu::ComputePipeline,
}

struct LevelBindings {
    forward: wgpu::BindGroup,
    backward: wgpu::BindGroup,
}

struct TransferBindings {
    roots: wgpu::BindGroup,
    weights: wgpu::BindGroup,
    restrict: wgpu::BindGroup,
    prolong: wgpu::BindGroup,
}

pub(super) struct CoarsePressure {
    levels: Vec<Level>,
    stages: Stages,
    smoothing: Vec<LevelBindings>,
    transfers: Vec<TransferBindings>,
    initialize: wgpu::BindGroup,
    export: wgpu::BindGroup,
}

impl CoarsePressure {
    pub(super) fn bind_fields(
        &self,
        device: &wgpu::Device,
        fields: PressureFields<'_>,
        uniform: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        bind(
            device,
            &self.stages.fine,
            &[
                (0, uniform),
                (1, fields.density),
                (2, fields.aperture_u),
                (3, fields.aperture_v),
                (4, fields.root_cell),
                (5, &self.levels[0].nodes),
            ],
        )
    }

    /// Rebuilds coefficients from this candidate's density and aperture fields.
    pub(super) fn prepare(&self, pass: &mut wgpu::ComputePass<'_>, fields: &wgpu::BindGroup) {
        dispatch(pass, &self.stages.fine, fields, self.levels[0].groups);
        for (level, transfer) in self.levels.iter().skip(1).zip(&self.transfers) {
            dispatch(pass, &self.stages.roots, &transfer.roots, level.groups);
            dispatch(pass, &self.stages.weights, &transfer.weights, level.groups);
        }
    }

    /// One fixed symmetric V-cycle with two pre- and two post-smoothing sweeps.
    /// Scratch starts at zero for each application, including every retry.
    pub(super) fn apply(&self, pass: &mut wgpu::ComputePass<'_>) {
        dispatch(
            pass,
            &self.stages.initialize,
            &self.initialize,
            self.levels[0].groups,
        );
        for index in 0..self.transfers.len() {
            self.smooth(pass, index);
            dispatch(
                pass,
                &self.stages.restrict,
                &self.transfers[index].restrict,
                self.levels[index + 1].groups,
            );
        }
        // The final 1 x 1 aggregate is the closed component's constant mode.
        // Its zero correction preserves the Neumann nullspace.
        for index in (0..self.transfers.len()).rev() {
            dispatch(
                pass,
                &self.stages.prolong,
                &self.transfers[index].prolong,
                self.levels[index].groups,
            );
            self.smooth(pass, index);
        }
        dispatch(
            pass,
            &self.stages.export,
            &self.export,
            self.levels[0].groups,
        );
    }

    fn smooth(&self, pass: &mut wgpu::ComputePass<'_>, index: usize) {
        let groups = self.levels[index].groups;
        let bindings = &self.smoothing[index];
        dispatch(pass, &self.stages.smooth, &bindings.forward, groups);
        dispatch(pass, &self.stages.smooth, &bindings.backward, groups);
    }
}
