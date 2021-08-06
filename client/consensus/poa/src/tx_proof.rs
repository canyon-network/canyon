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

use codec::Encode;

use sp_core::H256;
use sp_runtime::traits::Block as BlockT;

use canyon_primitives::ExtrinsicIndex;
use cp_consensus_poa::encode_index;
use cp_permastore::{TrieLayout, VerifyError};

use crate::trie::{prepare_trie_proof, TrieError};

/// Returns the calculated merkle proof given `extrinsic_index` and `extrinsics_root`.
///
/// # Panics
///
/// Panics if the calculated extrinsic root mismatches.
pub fn build_extrinsic_proof<Block: BlockT<Hash = canyon_primitives::Hash>>(
    extrinsic_index: ExtrinsicIndex,
    extrinsics_root: Block::Hash,
    extrinsics: Vec<Block::Extrinsic>,
) -> Result<Vec<Vec<u8>>, TrieError> {
    let leaves = extrinsics.iter().map(|xt| xt.encode()).collect::<Vec<_>>();

    let (db, root) = prepare_trie_proof(leaves);

    assert_eq!(
        extrinsics_root, root,
        "`extrinsics_root` mismatches the calculated root"
    );

    let proof = sp_trie::generate_trie_proof::<TrieLayout, _, _, _>(
        &db,
        extrinsics_root,
        &[encode_index(extrinsic_index)],
    )
    .map_err(|e| TrieError::Trie(Box::new(e)))?;

    Ok(proof)
}

/// A verifier for tx proof.
#[derive(Debug, Clone)]
pub struct TxProofVerifier<B: BlockT> {
    /// Recall extrinsic.
    recall_extrinsic: B::Extrinsic,
    /// Extrinsics root of recall block.
    extrinsics_root: B::Hash,
    /// Extrinsic index of `recall_extrinsic`.
    recall_extrinsic_index: ExtrinsicIndex,
}

impl<B: BlockT<Hash = canyon_primitives::Hash>> TxProofVerifier<B> {
    /// Creates a new instance of [`TxProofVerifier`].
    pub fn new(
        recall_extrinsic: B::Extrinsic,
        extrinsics_root: B::Hash,
        recall_extrinsic_index: ExtrinsicIndex,
    ) -> Self {
        Self {
            recall_extrinsic,
            extrinsics_root,
            recall_extrinsic_index,
        }
    }

    /// Returns Ok(()) if `tx_path` matches the inner tx proof.
    pub fn verify(&self, tx_path: &[Vec<u8>]) -> Result<(), VerifyError> {
        verify_extrinsic_proof(
            &self.extrinsics_root,
            self.recall_extrinsic_index,
            self.recall_extrinsic.encode(),
            tx_path,
        )
    }
}

/// Verify the extrinsic proof against the extrinsics root and related encoded extrinsic.
///
/// Returns Ok(()) if the extrinsic proof is valid.
pub fn verify_extrinsic_proof(
    extrinsics_root: &H256,
    extrinsic_index: ExtrinsicIndex,
    encoded_extrinsic: Vec<u8>,
    proof: &[Vec<u8>],
) -> Result<(), VerifyError> {
    sp_trie::verify_trie_proof::<TrieLayout, _, _, _>(
        extrinsics_root,
        proof,
        &[(encode_index(extrinsic_index), Some(encoded_extrinsic))],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_block_builder::{BlockBuilder, RecordProof};
    use sp_blockchain::HeaderBackend;
    use sp_keyring::AccountKeyring::{Alice, Bob};
    use substrate_test_runtime::{Block, Transfer};
    use substrate_test_runtime_client::{
        BlockBuilderExt, DefaultTestClientBuilderExt, TestClientBuilderExt,
    };

    #[test]
    fn test_extrinsic_proof() {
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

        let proof0 =
            build_extrinsic_proof::<Block>(0, extrinsics_root, extrinsics.clone()).unwrap();

        let proof1 =
            build_extrinsic_proof::<Block>(1, extrinsics_root, extrinsics.clone()).unwrap();

        assert!(verify_extrinsic_proof(
            &extrinsics_root,
            0,
            extrinsics[0].clone().encode(),
            &proof0
        )
        .is_ok());

        assert!(verify_extrinsic_proof(
            &extrinsics_root,
            0,
            extrinsics[1].clone().encode(),
            &proof0
        )
        .is_err());

        assert!(verify_extrinsic_proof(
            &extrinsics_root,
            1,
            extrinsics[1].clone().encode(),
            &proof1
        )
        .is_ok());
    }
}
