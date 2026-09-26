use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use llsp::dialect::Dialects;
use llsp::syntax::Tree;

mod common;

fn parse(c: &mut Criterion) {
    let dialects = Dialects::builtin();
    let mut group = c.benchmark_group("parse_1mb");
    for (name, template) in common::SAMPLES {
        let dialect = dialects.get(name).unwrap();
        let src = common::generate(template, 1 << 20);
        group.throughput(Throughput::Bytes(src.len() as u64));
        group.bench_function(*name, |b| {
            b.iter(|| Tree::parse(black_box(src.clone()), dialect))
        });
    }
    group.finish();
}

criterion_group!(benches, parse);
criterion_main!(benches);
