// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Canyon.
//
// Copyright (c) 2021 Canyon Labs.
//
// Canyon is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published
// by the Free Software Foundation, either version 3 of the License,
// or (at your option) any later version.
//
// Canyon is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Canyon. If not, see <http://www.gnu.org/licenses/>.

use super::*;
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;

use cp_consensus_poa::ProofOfAccess;
use sc_block_builder::{BlockBuilder, RecordProof};
use sp_blockchain::HeaderBackend;
use sp_keyring::AccountKeyring::{Alice, Bob};
use sp_runtime::traits::Block as BlockT;
use substrate_test_runtime::{Block, Transfer};
use substrate_test_runtime_client::{
    BlockBuilderExt, DefaultTestClientBuilderExt, TestClientBuilderExt,
};

use cc_consensus_poa::{build_extrinsic_proof, ChunkProof, ChunkProofBuilder};
use cp_permastore::CHUNK_SIZE;
use rand::Rng;

const MAX_DATA_SIZE: usize = 256 * 1024;

fn generate_chunk_proof(data: Vec<u8>, offset: u32) -> ChunkProof {
    ChunkProofBuilder::new(data, CHUNK_SIZE, offset)
        .build()
        .expect("Couldn't build chunk proof")
}

fn random_data(data_size: usize) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    (0..data_size).map(|_| rng.gen::<u8>()).collect()
}

fn mock_extrinsic_proof() -> Vec<Vec<u8>> {
    let builder = substrate_test_runtime_client::TestClientBuilder::new();
    let backend = builder.backend();
    let client = builder.build();

    let mut block_builder = BlockBuilder::new(
        &client,
        client.info().best_hash,
        client.info().best_number,
        RecordProof::Yes,
        Default::default(),
        &*backend,
    )
    .unwrap();

    block_builder
        .push_transfer(Transfer {
            from: Alice.into(),
            to: Bob.into(),
            amount: 123,
            nonce: 0,
        })
        .unwrap();

    block_builder
        .push_transfer(Transfer {
            from: Bob.into(),
            to: Alice.into(),
            amount: 1,
            nonce: 0,
        })
        .unwrap();

    let built_block = block_builder.build().unwrap();

    let (block, extrinsics) = built_block.block.deconstruct();

    let extrinsics_root = block.extrinsics_root;

    build_extrinsic_proof::<Block>(0, extrinsics_root, extrinsics.clone()).unwrap()
}

benchmarks! {
    // This will measure the execution time of `process_poa_outcome` for b in [1..1000] range.
    process_poa_outcome {
        let b in 1 .. 1000;
        let mut rng = rand::thread_rng();
        let offset = rng.gen_range(0..MAX_DATA_SIZE) as u32;
        let chunk_proof = generate_chunk_proof(random_data(MAX_DATA_SIZE), offset);
        let tx_proof = mock_extrinsic_proof();
        let poa = ProofOfAccess::new(1, tx_proof, chunk_proof);
        let poa_outcome = PoaOutcome::Justification(poa);
    }: process_poa_outcome (RawOrigin::None, poa_outcome)
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
