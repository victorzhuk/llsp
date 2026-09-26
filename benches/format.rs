use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use llsp::config::Format;
use llsp::dialect::Dialects;
use llsp::format::format;
use llsp::syntax::Tree;

mod common;

fn format_1mb(c: &mut Criterion) {
    let dialects = Dialects::builtin();
    let cfg = Format::default();
    let mut group = c.benchmark_group("format_1mb");
    for (name, template) in common::SAMPLES {
        let dialect = dialects.get(name).unwrap();
        let src = common::generate(template, 1 << 20);
        let tree = Tree::parse(src.clone(), dialect);
        group.throughput(Throughput::Bytes(src.len() as u64));
        group.bench_function(*name, |b| {
            b.iter(|| format(black_box(&tree), dialect, &cfg, &|_| None, None))
        });
    }
    group.finish();
}

criterion_group!(benches, format_1mb);
criterion_main!(benches);
