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
use sp_trie::TrieMut;

use cp_permastore::{Hasher, TrieLayout, VerifyError, CHUNK_SIZE};

pub fn encode_index(input: u32) -> Vec<u8> {
    codec::Encode::encode(&codec::Compact(input))
}

/// Unit type wrapper of the blake2_256 hash of a data chunk.
#[derive(Clone, Debug)]
pub struct ChunkId([u8; 32]);

impl From<[u8; 32]> for ChunkId {
    fn from(inner: [u8; 32]) -> Self {
        Self(inner)
    }
}

/// Converts the raw transaction data into a Vec of chunk id.
pub fn chunk_ids(tx_data: Vec<u8>) -> Vec<ChunkId> {
    tx_data
        .chunks(CHUNK_SIZE as usize)
        .map(|c| sp_io::hashing::blake2_256(c).into())
        .collect()
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Trie error.
    #[error(transparent)]
    Trie(#[from] Box<dyn std::error::Error + Send + Sync>),
}

/// This type represents the raw bytes of chunk as well as the chunk proof.
#[derive(Debug, Clone, Encode, Decode)]
pub struct ChunkProof {
    /// Random data chunk that is proved to exist.
    pub chunk: Vec<u8>,
    /// Index of `chunk`.
    pub chunk_index: u32,
    /// Trie nodes that compose the proof.
    ///
    /// Merkle path of chunks from `chunk` to the chunk root.
    pub proof: Vec<Vec<u8>>,
}

impl ChunkProof {
    /// Checks if the proof is valid against the chunk root.
    pub fn verify(&self, chunk_root: &H256) -> Result<(), VerifyError> {
        verify_chunk_proof(
            chunk_root,
            self.chunk.clone(),
            self.chunk_index,
            &self.proof,
        )
    }
}

/// Verifies the chunk given the `chunk_root` and `proof`.
pub fn verify_chunk_proof(
    chunk_root: &H256,
    chunk: Vec<u8>,
    chunk_index: u32,
    proof: &[Vec<u8>],
) -> Result<(), VerifyError> {
    sp_trie::verify_trie_proof::<TrieLayout, _, _, _>(
        chunk_root,
        proof,
        &[(encode_index(chunk_index), Some(chunk))],
    )
}

#[derive(Debug, Clone)]
pub struct ChunkProofBuilder {
    /// Raw bytes of transaction data.
    data: Vec<u8>,
    /// Size of per data chunk in bytes.
    chunk_size: usize,
    /// Index of the recall chunk.
    target_chunk_index: u32,
}

impl ChunkProofBuilder {
    /// Constructs a `ChunkProofBuilder`.
    pub fn new(data: Vec<u8>, chunk_size: usize, transaction_data_offset: usize) -> Self {
        debug_assert!(chunk_size > 0);

        let target_chunk_index = (transaction_data_offset / chunk_size) as u32;

        Self {
            data,
            chunk_size,
            target_chunk_index,
        }
    }

    /// Builds the chunk proof.
    pub fn build(&self) -> Result<ChunkProof, Error> {
        let mut target_chunk = Vec::with_capacity(self.chunk_size);

        let mut db = sp_trie::MemoryDB::<Hasher>::default();
        let mut chunk_root = sp_trie::empty_trie_root::<TrieLayout>();

        {
            let mut trie = sp_trie::TrieDBMut::<TrieLayout>::new(&mut db, &mut chunk_root);

            let chunks = self.data.chunks(self.chunk_size).map(|c| c.to_vec());

            for (index, chunk) in chunks.enumerate() {
                trie.insert(&encode_index(index as u32), &chunk)
                    .unwrap_or_else(|e| {
                        panic!("Failed to insert the trie node: {:?}, chunk index: {}", e, index)
                    });

                if index == self.target_chunk_index as usize {
                    target_chunk = chunk;
                }
            }

            trie.commit();
        }

        let proof = sp_trie::generate_trie_proof::<TrieLayout, _, _, _>(
            &db,
            chunk_root,
            &[encode_index(self.target_chunk_index)],
        )
        .map_err(|e| Error::Trie(Box::new(e)))?;

        Ok(ChunkProof {
            chunk: target_chunk,
            chunk_index: self.target_chunk_index,
            proof,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_proof() {
        use std::str::FromStr;

        let data = b"hello".to_vec();
        let chunk_proof_builder = ChunkProofBuilder::new(data, 1, 3);
        let chunk_proof = chunk_proof_builder.build().unwrap();
        let chunk_root = sp_core::H256::from_str(
            "0xf7c71b4df38c600bd0fd35ea9b3b5b23ff322ba638b0912d9270320f995f70eb",
        )
        .unwrap();

        assert!(verify_chunk_proof(&chunk_root, b"l".to_vec(), 3, &chunk_proof.proof).is_ok());
        assert!(verify_chunk_proof(&chunk_root, b"l".to_vec(), 4, &chunk_proof.proof).is_err());
    }
}
