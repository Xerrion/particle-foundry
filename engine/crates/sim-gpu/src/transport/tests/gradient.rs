use super::*;

#[test]
#[ignore = "requires a native GPU adapter"]
fn resolved_near_uniform_gradient_uses_cpu_geometric_branch() {
    let gpu = block_on(crate::GpuContext::new()).expect("native GPU adapter for E07 fixture");
    let grid = Grid::new(2.0, 2.0).unwrap();
    let fractions = [0.5, 0.500_000_5, 0.5, 0.5];
    let cell_volume = grid.cell_volume_m3();
    let liquid = fractions.map(|alpha| alpha * cell_volume * 1000.0);
    let carrier = fractions.map(|alpha| (1.0 - alpha) * cell_volume * 1.2);
    let initial = TransportInventory::new(
        grid,
        1000.0,
        1.2,
        liquid.to_vec(),
        carrier.to_vec(),
        vec![0.0; grid.cells()],
        vec![0.0; grid.cells()],
        vec![false; grid.cells()],
    )
    .unwrap();
    let mut u = vec![0.0; grid.u_faces()];
    let mut v = vec![0.0; grid.v_faces()];
    u[grid.u_face_index(1, 0).unwrap()] = 1.0;
    u[grid.u_face_index(1, 1).unwrap()] = -1.0;
    v[grid.v_face_index(0, 1).unwrap()] = -1.0;
    v[grid.v_face_index(1, 1).unwrap()] = 1.0;
    let mut u_aperture = vec![0.0; grid.u_faces()];
    let mut v_aperture = vec![0.0; grid.v_faces()];
    for y in 0..grid.height() {
        u_aperture[grid.u_face_index(1, y).unwrap()] = 1.0;
    }
    for x in 0..grid.width() {
        v_aperture[grid.v_face_index(x, 1).unwrap()] = 1.0;
    }
    let fields = PressureFields::new(
        grid,
        [Boundary::Closed; 4],
        vec![1000.0; grid.cells()],
        vec![0.0; grid.cells()],
        FaceValues {
            u: u.clone(),
            v: v.clone(),
        },
        FaceValues {
            u: u_aperture.clone(),
            v: v_aperture.clone(),
        },
    )
    .unwrap();
    let cpu = initial
        .candidate(
            &fields,
            &FaceValues {
                u: u.clone(),
                v: v.clone(),
            },
            0.0025,
        )
        .unwrap();

    let mass = liquid
        .into_iter()
        .chain(carrier)
        .map(|value| value as f32)
        .collect::<Vec<_>>();
    let marker = vec![0.0_f32; grid.cells() * 2];
    let headers = (0..grid.cells())
        .flat_map(|_| GpuCellHeader::new(0.0, 0).unwrap().to_le_bytes())
        .collect::<Vec<_>>();
    let to_f32 = |values: &[f64]| values.iter().map(|value| *value as f32).collect::<Vec<_>>();
    let mass_buffer = buffer(&gpu.device, "near-uniform mass", &f32_bytes(&mass));
    let marker_buffer = buffer(&gpu.device, "near-uniform marker", &f32_bytes(&marker));
    let header_buffer = buffer(&gpu.device, "near-uniform headers", &headers);
    let u_buffer = buffer(&gpu.device, "near-uniform u", &f32_bytes(&to_f32(&u)));
    let v_buffer = buffer(&gpu.device, "near-uniform v", &f32_bytes(&to_f32(&v)));
    let u_aperture_buffer = buffer(
        &gpu.device,
        "near-uniform u aperture",
        &f32_bytes(&to_f32(&u_aperture)),
    );
    let v_aperture_buffer = buffer(
        &gpu.device,
        "near-uniform v aperture",
        &f32_bytes(&to_f32(&v_aperture)),
    );
    let input = GpuTransportInput {
        mass_kg: &mass_buffer,
        marker: &marker_buffer,
        headers: &header_buffer,
        u_velocity_m_s: &u_buffer,
        v_velocity_m_s: &v_buffer,
        u_aperture: &u_aperture_buffer,
        v_aperture: &v_aperture_buffer,
    };
    let stage = GpuTransportStage::new(&gpu.device, grid).unwrap();
    let candidate = GpuTransportCandidate::new(&gpu.device, grid).unwrap();
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("near-uniform transport fixture"),
        });
    stage
        .encode(
            &gpu.device,
            &mut encoder,
            &input,
            &candidate,
            GpuTransportStep {
                dt_s: 0.0025,
                liquid_density_kg_m3: 1000.0,
                carrier_density_kg_m3: 1.2,
            },
        )
        .unwrap();
    gpu.queue.submit([encoder.finish()]);
    let status = read_bytes(&gpu.device, &gpu.queue, &candidate.status);
    assert_eq!(u32::from_le_bytes(status.try_into().unwrap()), 0);
    let actual_mass = read_f32(&gpu.device, &gpu.queue, &candidate.mass_kg);
    let max_alpha_error = (0..grid.cells())
        .map(|cell| {
            let gpu_alpha = actual_mass[cell] as f64 / 1000.0 / cell_volume;
            (gpu_alpha - cpu.inventory.alpha(cell).unwrap()).abs()
        })
        .fold(0.0_f64, f64::max);
    println!("near-uniform GPU alpha parity: max error {max_alpha_error:.3e}");
    assert!(max_alpha_error <= 1e-5, "frozen one-tick alpha parity gate");
    gpu.dispose();
}
