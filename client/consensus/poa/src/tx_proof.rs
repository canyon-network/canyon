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
use sp_runtime::traits::{Block as BlockT, Hash as HashT};
use sp_trie::TrieMut;

use cp_permastore::{Hasher, TrieLayout};

use crate::chunk_proof::{encode_index, Error};

pub fn build_transaction_proof<Block: BlockT<Hash = H256>>(
    extrinsic_index: usize,
    extrinsics_root: Block::Hash,
    extrinsics: Vec<Block::Extrinsic>,
) -> Result<Vec<Vec<u8>>, Error> {
    let mut db = sp_trie::MemoryDB::<Hasher>::default();
    let mut calc_extrinsics_root = sp_trie::empty_trie_root::<TrieLayout>();

    {
        let mut trie = sp_trie::TrieDBMut::<TrieLayout>::new(&mut db, &mut calc_extrinsics_root);

        for (index, extrinsic) in extrinsics.iter().enumerate() {
            trie.insert(&encode_index(index as u32), &extrinsic.encode())
                .unwrap_or_else(|e| {
                    panic!(
                        "Failed to insert the trie node: {:?}, extrinsic index: {}",
                        e, index
                    )
                });
        }

        trie.commit();
    }

    assert_eq!(extrinsics_root, calc_extrinsics_root);

    let proof = sp_trie::generate_trie_proof::<TrieLayout, _, _, _>(
        &db,
        extrinsics_root,
        &[encode_index(extrinsic_index as u32)],
    )
    .map_err(|e| Error::Trie(Box::new(e)))?;

    Ok(proof)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_block_builder::{BlockBuilder, RecordProof};
    use sp_blockchain::HeaderBackend;
    use sp_core::Blake2Hasher;
    use sp_keyring::AccountKeyring::{Alice, Bob};
    use sp_state_machine::Backend;
    use substrate_test_runtime::Transfer;
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

        block_builder.push_transfer(Transfer {
            from: Alice.into(),
            to: Bob.into(),
            amount: 123,
            nonce: 1,
        });

        let block = block_builder.build().unwrap();

        let proof = block.proof.expect("Proof is build on request");

        let backend = sp_state_machine::create_proof_check_backend::<Blake2Hasher>(
            block.storage_changes.transaction_storage_root,
            proof,
        )
        .unwrap();

        assert!(backend
            .storage(&sp_core::storage::well_known_keys::CODE)
            .unwrap_err()
            .contains("Database missing expected key"));
    }
}
