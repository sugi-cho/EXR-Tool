use exrtool_core::parse_cube;

#[test]
fn lut_1d_identity() {
    let cube = "TITLE \"id\"\nLUT_1D_SIZE 2\nDOMAIN_MIN 0.0 0.0 0.0\nDOMAIN_MAX 1.0 1.0 1.0\n0.0 0.0 0.0\n1.0 1.0 1.0\n";
    let lut = parse_cube(cube).expect("parse 1D LUT");
    let c = [0.2_f32, 0.4_f32, 0.8_f32];
    let out = lut.apply(c);
    for i in 0..3 {
        assert!((out[i] - c[i]).abs() < 1e-6);
    }
}

#[test]
fn lut_3d_identity() {
    let cube = "TITLE \"id3d\"\nLUT_3D_SIZE 2\nDOMAIN_MIN 0.0 0.0 0.0\nDOMAIN_MAX 1.0 1.0 1.0\n0.0 0.0 0.0\n1.0 0.0 0.0\n0.0 1.0 0.0\n1.0 1.0 0.0\n0.0 0.0 1.0\n1.0 0.0 1.0\n0.0 1.0 1.0\n1.0 1.0 1.0\n";
    let lut = parse_cube(cube).expect("parse 3D LUT");
    let c = [0.2_f32, 0.3_f32, 0.4_f32];
    let out = lut.apply(c);
    for i in 0..3 {
        assert!((out[i] - c[i]).abs() < 1e-6);
    }
}
