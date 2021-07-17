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

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};

use sp_inherents::InherentIdentifier;
use sp_runtime::ConsensusEngineId;
use sp_std::vec::Vec;

/// The identifier for the inherent of poa pallet.
pub const POA_INHERENT_IDENTIFIER: InherentIdentifier = *b"poaproof";

/// The engine id for the Proof of Access consensus.
pub const POA_ENGINE_ID: ConsensusEngineId = *b"POA:";

/// This struct includes the raw bytes of recall chunk as well as the chunk proof stuffs.
#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub struct ChunkProof {
    /// Random data chunk that is proved to exist.
    pub chunk: Vec<u8>,
    /// Index of `chunk` in the total chunks of that transaction data.
    ///
    /// Required for verifing `proof`.
    pub chunk_index: u32,
    /// Trie nodes that compose the proof.
    ///
    /// Merkle path of chunks from `chunk` to the chunk root.
    pub proof: Vec<Vec<u8>>,
}

/// An utility function to enocde chunk/extrinsic index as trie key.
// The final proof can be more compact.
// See https://github.com/paritytech/substrate/pull/8624#discussion_r616075183
pub fn encode_index(input: u32) -> Vec<u8> {
    codec::Encode::encode(&codec::Compact(input))
}

impl ChunkProof {
    /// Returns the proof size in bytes.
    pub fn size(&self) -> usize {
        self.proof.iter().map(|p| p.len()).sum()
    }

    /// Calculates the merkle root of the raw data `chunk`.
    #[cfg(feature = "std")]
    pub fn chunk_root(&self, chunk_size: usize) -> sp_core::H256 {
        use sp_core::Blake2Hasher;
        use sp_io::hashing::blake2_256;
        use sp_trie::TrieMut;

        let mut db = sp_trie::MemoryDB::<Blake2Hasher>::default();
        let mut chunk_root = sp_trie::empty_trie_root::<sp_trie::Layout<Blake2Hasher>>();

        {
            let mut trie =
                sp_trie::TrieDBMut::<sp_trie::Layout<Blake2Hasher>>::new(&mut db, &mut chunk_root);

            let chunks = self.chunk.chunks(chunk_size).map(|c| c.to_vec());

            for (index, chunk) in chunks.enumerate() {
                trie.insert(&encode_index(index as u32), &blake2_256(&chunk))
                    .unwrap_or_else(|e| {
                        panic!(
                            "Failed to insert the trie node: {:?}, chunk index: {}",
                            e, index
                        )
                    });
            }

            trie.commit();
        }

        chunk_root
    }
}

/// This struct is used to prove the random historical data access of block author.
#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub struct ProofOfAccess {
    /// Number of trials when a valid `ProofOfAccess` created.
    pub depth: u32,
    /// Merkle path/proof of the recall tx.
    pub tx_path: Vec<Vec<u8>>,
    /// Proof of the recall chunk.
    pub chunk_proof: ChunkProof,
}

impl ProofOfAccess {
    /// Creates a new instance of [`ProofOfAccess`].
    pub fn new(depth: u32, tx_path: Vec<Vec<u8>>, chunk_proof: ChunkProof) -> Self {
        Self {
            depth,
            tx_path,
            chunk_proof,
        }
    }
}

/// This struct represents the outcome of creating the inherent data of [`ProofOfAccess`].
#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub enum PoaOutcome {
    /// Not required for this block due to the entire weave is empty.
    Skipped,
    /// Failed to create a valid [`ProofOfAccess`] due to the maximum depth limit has been reached.
    MaxDepthReached,
    /// Generate a [`ProofOfAccess`] successfully.
    ///
    /// Each block contains a justification of poa as long as the weave
    /// size is not 0 and will be verified on block import.
    Justification(ProofOfAccess),
}

impl PoaOutcome {
    /// Returns true if the poa inherent must be included.
    pub fn require_inherent(&self) -> bool {
        matches!(self, Self::Justification(..))
    }
}
