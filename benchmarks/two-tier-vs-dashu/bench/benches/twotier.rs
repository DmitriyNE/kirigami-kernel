use criterion::{Criterion, black_box, criterion_group, criterion_main};
use twotier_bench::*;

const N: usize = 2000;

fn int_dot(c: &mut Criterion) {
    let a = small_ints(N, 0x1111, 1_000);
    let b = small_ints(N, 0x2222, 1_000);
    let (at, bt) = (twotier_ints(&a), twotier_ints(&b));
    let (ad, bd) = (dashu_ints(&a), dashu_ints(&b));
    let mut g = c.benchmark_group("int_dot(small)");
    g.bench_function("two_tier", |bn| bn.iter(|| black_box(int_dot_twotier(&at, &bt))));
    g.bench_function("dashu_only", |bn| bn.iter(|| black_box(int_dot_dashu(&ad, &bd))));
    g.finish();
}

fn rat_det(c: &mut Criterion) {
    let q = small_rats(4 * N, 0x3333, 500);
    let (qt, qd) = (twotier_rats(&q), dashu_rats(&q));
    let mut g = c.benchmark_group("rat_2x2_det(small)");
    g.bench_function("two_tier", |bn| bn.iter(|| black_box(rat_det_twotier(&qt))));
    g.bench_function("dashu_only", |bn| bn.iter(|| black_box(rat_det_dashu(&qd))));
    g.finish();
}

fn rat_cmp(c: &mut Criterion) {
    let q = small_rats(2 * N, 0x4444, 500);
    let (qt, qd) = (twotier_rats(&q), dashu_rats(&q));
    let mut g = c.benchmark_group("rat_cmp(small)");
    g.bench_function("two_tier", |bn| bn.iter(|| black_box(rat_cmp_twotier(&qt))));
    g.bench_function("dashu_only", |bn| bn.iter(|| black_box(rat_cmp_dashu(&qd))));
    g.finish();
}

fn crossover(c: &mut Criterion) {
    // 60 multipliers in [2,10]: the product overflows i128 (~2^127) around index 40,
    // so ~2/3 of the ops run in bignum for BOTH — this measures the two-tier overhead
    // (overflow-check + promote) versus staying in dashu the whole time.
    let m = small_mults(60, 0x5555);
    let (mt, md) = (twotier_ints(&m), dashu_ints(&m));
    let mut g = c.benchmark_group("crossover_product(overflows i128)");
    g.bench_function("two_tier", |bn| bn.iter(|| black_box(crossover_twotier(&mt))));
    g.bench_function("dashu_only", |bn| bn.iter(|| black_box(crossover_dashu(&md))));
    g.finish();
}

criterion_group!(benches, int_dot, rat_det, rat_cmp, crossover);
criterion_main!(benches);
