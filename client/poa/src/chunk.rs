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

use codec::{Decode, Encode};

use sp_core::H256;
use sp_runtime::traits::Hash as HashT;
use sp_trie::TrieMut;

use cp_permastore::{Hasher, TrieLayout, DEFAULT_CHUNK_SIZE};

pub struct ChunkId([u8; 32]);

impl From<[u8; 32]> for ChunkId {
    fn from(inner: [u8; 32]) -> Self {
        Self(inner)
    }
}

pub struct Chunk(Vec<u8>);

impl From<Vec<u8>> for Chunk {
    fn from(inner: Vec<u8>) -> Self {
        Self(inner)
    }
}

pub fn as_chunk_ids(tx_data: Vec<u8>) -> Vec<ChunkId> {
    tx_data
        .chunks(DEFAULT_CHUNK_SIZE as usize)
        .map(|c| sp_io::hashing::blake2_256(c).into())
        .collect()
}

pub fn encode_index(input: u32) -> Vec<u8> {
    codec::Encode::encode(&codec::Compact(input))
}

#[derive(Debug, Clone)]
pub struct TrieError(String);

pub fn build_extrinsic_proof<Hash: HashT>(
    extrinsic_index: usize,
    extrinsics_root: Hash::Output,
) -> Result<Vec<Vec<u8>>, TrieError> {
    todo!()
}

/// Proof and full content of recall chunk.
#[derive(Debug, Clone, Encode, Decode)]
pub struct ChunkProof {
    /// Random data chunk that is proved to exist.
    pub chunk: Vec<u8>,
    /// Index of `chunk`.
    pub chunk_index: u32,
    /// Trie nodes that compose the proof.
    ///
    /// Merkle path of data chunks from the recall chunk to the data root.
    pub proof: Vec<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct VerifyError(String);

impl ChunkProof {
    /// Checks if the proof is valid given the data root.
    pub fn verify(&self, data_root: &H256) -> Result<(), VerifyError> {
        verify_chunk_proof(data_root, self.chunk.clone(), self.chunk_index, &self.proof)
    }
}

pub fn verify_chunk_proof(
    data_root: &H256,
    chunk: Vec<u8>,
    chunk_index: u32,
    proof: &[Vec<u8>],
) -> Result<(), VerifyError> {
    sp_trie::verify_trie_proof::<TrieLayout, _, _, _>(
        data_root,
        proof,
        &[(encode_index(chunk_index), Some(chunk))],
    )
    .map_err(|e| VerifyError(format!("{:?}", e)))
}

#[derive(Debug, Clone)]
pub struct ChunkProofBuilder {
    /// Raw bytes of transaction data.
    tx_data: Vec<u8>,
    /// Size of per data chunk in bytes.
    chunk_size: usize,
    /// Index of recall chunk.
    recall_chunk_index: u32,
}

impl ChunkProofBuilder {
    pub fn new(tx_data: Vec<u8>, chunk_size: usize, tx_offset: usize) -> Self {
        debug_assert!(chunk_size > 0);

        let recall_chunk_index = (tx_offset / chunk_size) as u32;

        Self {
            tx_data,
            chunk_size,
            recall_chunk_index,
        }
    }

    /// Constructs a `ChunkProof`.
    ///
    /// TODO: use MMR?
    pub fn build(&self) -> Result<ChunkProof, TrieError> {
        let mut recall_chunk: Vec<u8> = Vec::with_capacity(self.chunk_size);

        let mut db = sp_trie::MemoryDB::<Hasher>::default();
        let mut data_root = sp_trie::empty_trie_root::<TrieLayout>();

        {
            let mut trie = sp_trie::TrieDBMut::<TrieLayout>::new(&mut db, &mut data_root);

            let chunks = self.tx_data.chunks(self.chunk_size).map(|c| c.to_vec());

            for (index, chunk) in chunks.enumerate() {
                trie.insert(&encode_index(index as u32), &chunk)
                    .unwrap_or_else(|e| panic!("Trie error: {:?}", e));

                if index == self.recall_chunk_index as usize {
                    recall_chunk = chunk;
                }
            }

            trie.commit();
        }

        let proof = sp_trie::generate_trie_proof::<TrieLayout, _, _, _>(
            &db,
            data_root,
            &[encode_index(self.recall_chunk_index)],
        )
        .map_err(|e| TrieError(format!("{:?}", e)))?;

        Ok(ChunkProof {
            chunk: recall_chunk,
            chunk_index: self.recall_chunk_index,
            proof,
        })
    }
}

#[test]
fn test_build_proof() {
    use std::str::FromStr;

    let tx_data = b"hello".to_vec();

    let chunk_proof_builder = ChunkProofBuilder::new(tx_data, 1, 3);

    let chunk_proof = chunk_proof_builder.build().unwrap();

    let data_root = sp_core::H256::from_str(
        "0xf7c71b4df38c600bd0fd35ea9b3b5b23ff322ba638b0912d9270320f995f70eb",
    )
    .unwrap();

    assert!(verify_chunk_proof(&data_root, b"l".to_vec(), 3, &chunk_proof.proof).is_ok());
    assert!(verify_chunk_proof(&data_root, b"l".to_vec(), 4, &chunk_proof.proof).is_err());
}
