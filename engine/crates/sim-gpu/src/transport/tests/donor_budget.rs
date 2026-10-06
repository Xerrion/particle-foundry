use super::*;

#[test]
#[ignore = "requires a native Metal GPU"]
fn metal_two_outlet_budget_preserves_phase_and_marker_on_both_axes() {
    let gpu = block_on(crate::GpuContext::new()).expect("native GPU adapter");
    assert_eq!(gpu.backend(), "Metal");
    let mass = [0.001_f32, 0.0009999996, 0.001, 0.0, 2.7105054e-20, 0.0];
    let marker = [0.25_f32, 0.24999982, 0.25, 0.0, 8.881784e-16, 0.0];
    let mut provisional = vec![0.0_f32; 4 * 5];
    provisional[5..10].copy_from_slice(&[
        -3.0224773e-13,
        -3.0224767e-10,
        -2.7105054e-20,
        -7.556189e-8,
        -8.881784e-16,
    ]);
    provisional[10..15].copy_from_slice(&[
        4.856996e-13,
        4.856995e-10,
        2.7105054e-20,
        1.2142483e-7,
        8.881784e-16,
    ]);
    assert_two_outlet_budget(&gpu, &mass, &marker, &provisional, true, false);
    gpu.dispose();
}

#[test]
#[ignore = "requires a native Metal GPU"]
fn metal_two_outlet_budget_accepts_rounded_sum_after_carrier_limit() {
    let gpu = block_on(crate::GpuContext::new()).expect("native GPU adapter");
    assert_eq!(gpu.backend(), "Metal");
    let owned_carrier = f32::from_bits(0x2499_999a);
    let measured_negative = f32::from_bits(0x2214_f209);
    let measured_positive = f32::from_bits(0x2494_f20a);
    // The sustained failure's corrected ledger consumes exactly the
    // rounded inventory. The old subtraction guard rejects its smaller
    // share because the separately rounded remainder is lower.
    assert_eq!(measured_negative + measured_positive, owned_carrier);
    assert_eq!((owned_carrier - measured_positive).to_bits(), 0x2214_f200);
    assert!(measured_negative > owned_carrier - measured_positive);
    let mass = [
        0.001_f32,
        f32::from_bits(0x3a83_1276),
        0.001,
        0.0,
        owned_carrier,
        0.0,
    ];
    let marker = [0.25_f32, 0.25, 0.25, 0.0, 0.125, 0.0];
    let mut provisional = vec![0.0_f32; 4 * 5];
    for (face, sign, volume_bits, liquid_bits, carrier_bits) in [
        (1_usize, -1.0_f32, 0x2dd1_8900, 0x32cc_9fc8, 0x2219_999a),
        (2_usize, 1.0_f32, 0x3071_4581, 0x356b_9dde, 0x2499_999a),
    ] {
        let liquid = f32::from_bits(liquid_bits);
        let carrier = f32::from_bits(carrier_bits);
        provisional[face * 5..face * 5 + 5].copy_from_slice(&[
            sign * f32::from_bits(volume_bits),
            sign * liquid,
            sign * carrier,
            sign * marker[1] * (liquid / mass[1]),
            sign * marker[4] * (carrier / owned_carrier),
        ]);
    }
    assert!(-provisional[7] + provisional[12] > owned_carrier);
    // The measured state is between directional sweeps. Its shared-face
    // budget must pass on either axis; an isolated Y gather must still
    // reject its unfinished volume closure.
    assert_two_outlet_budget(&gpu, &mass, &marker, &provisional, true, true);
    gpu.dispose();
}

#[test]
#[ignore = "requires a native Metal GPU"]
fn metal_two_outlet_marker_budget_matches_rounded_phase_exhaustion() {
    let gpu = block_on(crate::GpuContext::new()).expect("native GPU adapter");
    assert_eq!(gpu.backend(), "Metal");
    let tiny_mass = 1.9347336e-21_f32;
    let tiny_marker = 4.8368337e-19_f32;
    let measured_mass = f32::from_bits(0x2494_cccd);
    let measured_marker = f32::from_bits(0x2c3c_6a35);
    for (owned_mass, owned_marker, negative, positive, exhausts_carrier) in [
        (
            tiny_mass,
            tiny_marker,
            5.804394e-22_f32,
            1.3542942e-21_f32,
            true,
        ),
        (
            tiny_mass,
            tiny_marker,
            1.1608401e-25_f32,
            1.9346175e-21_f32,
            true,
        ),
        (
            tiny_mass,
            tiny_marker,
            tiny_mass * 0.25,
            tiny_mass * 0.5,
            false,
        ),
        // Exact normal carrier amounts from the sustained browser failure.
        (
            measured_mass,
            measured_marker,
            f32::from_bits(0x2374_2f44),
            f32::from_bits(0x246c_8dc9),
            true,
        ),
        // Nearby face amounts produce the measured provisional marker
        // share with host division rounding as well. Both phase sums still
        // exhaust inventory. Cancelling the recomplement leaves one ULP.
        (
            measured_mass,
            measured_marker,
            f32::from_bits(0x2374_2f45),
            f32::from_bits(0x246c_8dc8),
            true,
        ),
    ] {
        // These fixtures do not need phase overdraw repair. Independent
        // face marker products can nevertheless leave an orphan marker
        // when gather rounds the two outgoing phase masses to inventory.
        assert!(negative <= owned_mass - positive);
        assert_eq!(negative + positive == owned_mass, exhausts_carrier);
        let mass = [0.001_f32, 0.001, 0.001, 0.0, owned_mass, 0.0];
        let marker = [0.25_f32, 0.25, 0.25, 0.0, owned_marker, 0.0];
        let mut provisional = vec![0.0_f32; 4 * 5];
        for (face, sign, volume, carrier) in [
            (1_usize, -1.0_f32, 3.0224773e-13_f32, negative),
            (2_usize, 1.0_f32, 4.856996e-13_f32, positive),
        ] {
            let liquid = (volume - carrier / 1.2) * 1000.0;
            provisional[face * 5..face * 5 + 5].copy_from_slice(&[
                sign * volume,
                sign * liquid,
                sign * carrier,
                sign * marker[1] * (liquid / mass[1]),
                sign * owned_marker * (carrier / owned_mass),
            ]);
        }
        assert_two_outlet_budget(&gpu, &mass, &marker, &provisional, exhausts_carrier, false);
    }
    gpu.dispose();
}
