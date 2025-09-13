use criterion::{black_box, criterion_group, criterion_main, Criterion};
use exrtool_core::{make_3d_lut_cube, Primaries, TransferFn};

fn bench_lut_gen(c: &mut Criterion) {
    let mut group = c.benchmark_group("make_3d_lut_cube");
    for &size in &[17usize, 33, 65] {
        group.bench_function(format!("size {size}"), |b| {
            b.iter(|| {
                let _ = black_box(make_3d_lut_cube(
                    Primaries::SrgbD65,
                    TransferFn::Srgb,
                    Primaries::Rec2020D65,
                    TransferFn::Srgb,
                    size,
                ));
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_lut_gen);
criterion_main!(benches);
