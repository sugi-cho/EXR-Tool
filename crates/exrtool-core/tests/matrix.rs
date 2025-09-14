use exrtool_core::{make_3d_lut_cube, parse_cube, Primaries, TransferFn};
use nalgebra::{Matrix3, Vector3};

fn xy_to_xyz(x: f64, y: f64) -> Vector3<f64> {
    Vector3::new(x / y, 1.0, (1.0 - x - y) / y)
}

fn primaries_data(p: Primaries) -> (Vector3<f64>, Vector3<f64>, Vector3<f64>, Vector3<f64>) {
    match p {
        Primaries::SrgbD65 => (
            xy_to_xyz(0.640, 0.330),
            xy_to_xyz(0.300, 0.600),
            xy_to_xyz(0.150, 0.060),
            xy_to_xyz(0.3127, 0.3290),
        ),
        Primaries::Rec2020D65 => (
            xy_to_xyz(0.708, 0.292),
            xy_to_xyz(0.170, 0.797),
            xy_to_xyz(0.131, 0.046),
            xy_to_xyz(0.3127, 0.3290),
        ),
        Primaries::ACEScgD60 => (
            xy_to_xyz(0.713, 0.293),
            xy_to_xyz(0.165, 0.830),
            xy_to_xyz(0.128, 0.044),
            xy_to_xyz(0.32168, 0.33767),
        ),
        Primaries::ACES2065_1D60 => (
            xy_to_xyz(0.73470, 0.26530),
            xy_to_xyz(0.00000, 1.00000),
            xy_to_xyz(0.00010, -0.07700),
            xy_to_xyz(0.32168, 0.33767),
        ),
    }
}

fn rgb_to_rgb_matrix_manual(src: Primaries, dst: Primaries) -> Matrix3<f64> {
    let (xr_s, xg_s, xb_s, w_s) = primaries_data(src);
    let m_src = Matrix3::from_columns(&[xr_s, xg_s, xb_s]);
    let s_src = m_src.try_inverse().unwrap() * w_s;
    let m_src = m_src * Matrix3::from_diagonal(&s_src);

    let (xr_d, xg_d, xb_d, w_d) = primaries_data(dst);
    let m_dst = Matrix3::from_columns(&[xr_d, xg_d, xb_d]);
    let s_dst = m_dst.try_inverse().unwrap() * w_d;
    let m_dst = m_dst * Matrix3::from_diagonal(&s_dst);

    m_dst.try_inverse().unwrap() * m_src
}

#[test]
fn matrix_srgb_to_rec2020_matches_manual() {
    let lut_str = make_3d_lut_cube(
        Primaries::SrgbD65,
        TransferFn::Linear,
        Primaries::Rec2020D65,
        TransferFn::Linear,
        2,
        0,
    );
    let lut = parse_cube(&lut_str).unwrap();
    let m = rgb_to_rgb_matrix_manual(Primaries::SrgbD65, Primaries::Rec2020D65);

    let basis = [[1.0_f32, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for b in basis.iter() {
        let out = lut.apply(*b);
        let v = Vector3::new(b[0] as f64, b[1] as f64, b[2] as f64);
        let expected = m * v;
        assert!((out[0] as f64 - expected.x).abs() < 1e-6);
        assert!((out[1] as f64 - expected.y).abs() < 1e-6);
        assert!((out[2] as f64 - expected.z).abs() < 1e-6);
    }
}
