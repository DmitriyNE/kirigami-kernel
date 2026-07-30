//! criterion bench of the deg-12 Sturm PRS yardstick, one function per
//! rational-capable backend. `sample_size(10)` bounds the ~17s-per-run
//! num-rational backend; dashu/malachite are sub-second. Naive Euclidean PRS
//! deliberately triggers Sturm coefficient explosion — that IS the bignum stress.

use criterion::{Criterion, criterion_group, criterion_main};
use selector::backends::{Dashu, Malachite, Num};
use selector::{make_poly, prs::sturm_root_count};
use std::hint::black_box;

fn prs_yardstick(c: &mut Criterion) {
    let mut g = c.benchmark_group("deg12-sturm-prs-256bit");
    g.sample_size(10);

    let pd = make_poly::<Dashu>();
    g.bench_function("dashu", |b| b.iter(|| sturm_root_count(black_box(&pd))));

    let pm = make_poly::<Malachite>();
    g.bench_function("malachite", |b| b.iter(|| sturm_root_count(black_box(&pm))));

    let pn = make_poly::<Num>();
    g.bench_function("num-rational", |b| {
        b.iter(|| sturm_root_count(black_box(&pn)))
    });

    g.finish();
}

criterion_group!(benches, prs_yardstick);
criterion_main!(benches);
