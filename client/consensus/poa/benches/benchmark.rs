use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::Rng;

use cc_consensus_poa::{ChunkProof, ChunkProofBuilder};
use cp_permastore::CHUNK_SIZE;

fn generate_chunk_proof(data: Vec<u8>, offset: u32) -> ChunkProof {
    ChunkProofBuilder::new(data, CHUNK_SIZE, offset)
        .build()
        .expect("failed to build chunk proof")
}

fn random_data(data_size: usize) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    (0..data_size).map(|_| rng.gen::<u8>()).collect()
}

fn chunk_proof_benchmark(c: &mut Criterion) {
    let data = random_data(10 * 1024 * 1024);
    c.bench_function("chunk proof generation 10MiB", |b| {
        b.iter(|| generate_chunk_proof(black_box(data.clone()), black_box(20)))
    });

    let data = random_data(100 * 1024 * 1024);
    c.bench_function("chunk proof generation 100MiB", |b| {
        b.iter(|| generate_chunk_proof(black_box(data.clone()), black_box(20)))
    });

    let data = random_data(1024 * 1024 * 1024);
    c.bench_function("chunk proof generation 1GiB", |b| {
        b.iter(|| generate_chunk_proof(black_box(data.clone()), black_box(20)))
    });
}

criterion_group!(benches, chunk_proof_benchmark);
criterion_main!(benches);
