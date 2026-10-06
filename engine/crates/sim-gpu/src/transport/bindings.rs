pub(super) fn closure_group(
    device: &wgpu::Device,
    pipeline: &wgpu::ComputePipeline,
    uniform: &wgpu::Buffer,
    storage: &[&wgpu::Buffer],
) -> wgpu::BindGroup {
    let mut entries = vec![wgpu::BindGroupEntry {
        binding: 0,
        resource: uniform.as_entire_binding(),
    }];
    entries.extend(
        storage
            .iter()
            .enumerate()
            .map(|(index, buffer)| wgpu::BindGroupEntry {
                binding: index as u32 + 1,
                resource: buffer.as_entire_binding(),
            }),
    );
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("transport shared-face closure bindings"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &entries,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn face_group(
    device: &wgpu::Device,
    pipeline: &wgpu::ComputePipeline,
    uniform: &wgpu::Buffer,
    mass: &wgpu::Buffer,
    marker: &wgpu::Buffer,
    headers: &wgpu::Buffer,
    velocity: &wgpu::Buffer,
    aperture: &wgpu::Buffer,
    flux: &wgpu::Buffer,
    status: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("transport face bindings"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: mass.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: marker.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: headers.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: velocity.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: aperture.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: flux.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: status.as_entire_binding(),
            },
        ],
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn budget_group(
    device: &wgpu::Device,
    pipeline: &wgpu::ComputePipeline,
    uniform: &wgpu::Buffer,
    mass: &wgpu::Buffer,
    marker: &wgpu::Buffer,
    provisional: &wgpu::Buffer,
    corrected: &wgpu::Buffer,
    status: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("transport donor budget bindings"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: mass.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: marker.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: provisional.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: corrected.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: status.as_entire_binding(),
            },
        ],
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn gather_group(
    device: &wgpu::Device,
    pipeline: &wgpu::ComputePipeline,
    uniform: &wgpu::Buffer,
    mass: &wgpu::Buffer,
    marker: &wgpu::Buffer,
    headers: &wgpu::Buffer,
    flux: &wgpu::Buffer,
    output_mass: &wgpu::Buffer,
    output_marker: &wgpu::Buffer,
    status: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("transport gather bindings"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: mass.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: marker.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: headers.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: flux.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: output_mass.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: output_marker.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: status.as_entire_binding(),
            },
        ],
    })
}
