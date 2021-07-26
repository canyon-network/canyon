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

use crate::mock::{new_test_ext, Poa, Test};
use crate::{DepthInfo, HistoryDepth, TestAuthor};

fn generate_chunk_proof(data: Vec<u8>, offset: u32) -> ChunkProof {
    ChunkProofBuilder::new(data, CHUNK_SIZE, offset)
        .build()
        .expect("Couldn't build chunk proof")
}

fn mock_extrinsic_proof() -> Vec<Vec<u8>> {
    let builder = substrate_test_runtime_client::TestClientBuilder::new();
    let backend = builder.backend();
    let client = builder.build();

    let mut block_builder = BlockBuilder::new(
        &client,
        client.info().best_hash,
        client.info().best_number,
        RecordProof::No,
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

#[test]
fn test_generate_proof_of_access() {
    let random_data = crate::benchmarking::mock_a_data_chunk();
    let offset = 56_780_000;
    let chunk_proof = generate_chunk_proof(random_data, offset);

    println!("chunk_index: {:?}", chunk_proof.chunk_index);
    println!("chunk_proof: {:?}", chunk_proof.proof);

    let tx_proof = mock_extrinsic_proof();

    println!("tx_proof: {:?}", tx_proof);
}

#[test]
fn note_depth_should_work() {
    new_test_ext().execute_with(|| {
        TestAuthor::<Test>::put(6);
        Poa::note_depth(10);
        assert_eq!(
            HistoryDepth::<Test>::get(&6).unwrap(),
            DepthInfo {
                total_depth: 10,
                blocks: 1
            }
        );

        TestAuthor::<Test>::put(8);
        Poa::note_depth(1);
        assert_eq!(
            HistoryDepth::<Test>::get(&8).unwrap(),
            DepthInfo {
                total_depth: 1,
                blocks: 1
            }
        );

        TestAuthor::<Test>::put(6);
        Poa::note_depth(1);
        assert_eq!(
            HistoryDepth::<Test>::get(&6).unwrap(),
            DepthInfo {
                total_depth: 11,
                blocks: 2
            }
        );
    });
}
