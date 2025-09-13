use criterion::{black_box, criterion_group, criterion_main, Criterion};
use exrtool_core::{make_3d_lut_cube, Primaries, TransferFn};

fn bench_make_3d_lut(c: &mut Criterion) {
    c.bench_function("make_3d_lut_cube", |b| {
        b.iter(|| {
            make_3d_lut_cube(
                Primaries::SrgbD65,
                TransferFn::Srgb,
                Primaries::Rec2020D65,
                TransferFn::Srgb,
                black_box(33),
            )
        })
    });
}

criterion_group!(benches, bench_make_3d_lut);
criterion_main!(benches);
