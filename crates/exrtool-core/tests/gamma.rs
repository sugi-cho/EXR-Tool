use exrtool_core::apply_gamma;

#[test]
fn gamma_applies_inverse_power() {
    let rgb = [0.25_f32, 0.25_f32, 0.25_f32];
    let out = apply_gamma(rgb, 2.0);
    assert!((out[0] - 0.5).abs() < 1e-6);
    assert!((out[1] - 0.5).abs() < 1e-6);
    assert!((out[2] - 0.5).abs() < 1e-6);
}

#[test]
fn gamma_zero_is_identity() {
    let rgb = [0.1_f32, 0.2_f32, 0.3_f32];
    let out = apply_gamma(rgb, 0.0);
    assert_eq!(out, rgb);
}
