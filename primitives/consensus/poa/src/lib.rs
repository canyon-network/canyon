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

use codec::{Decode, Encode, MaxEncodedLen};

use sp_inherents::InherentIdentifier;
use sp_runtime::ConsensusEngineId;
use sp_std::vec::Vec;

/// The identifier for the inherent of poa pallet.
pub const POA_INHERENT_IDENTIFIER: InherentIdentifier = *b"poaproof";

/// The engine id for the Proof of Access consensus.
pub const POA_ENGINE_ID: ConsensusEngineId = *b"POA:";

/// This struct includes the raw bytes of recall chunk as well as the chunk proof stuffs.
#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
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
    /// Creates a new instance of [`ChunkProof`].
    pub fn new(chunk: Vec<u8>, chunk_index: u32, proof: Vec<Vec<u8>>) -> Self {
        Self {
            chunk,
            chunk_index,
            proof,
        }
    }

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
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
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

    /// Returns the size of tx proof.
    pub fn tx_path_len(&self) -> usize {
        self.tx_path.iter().map(|x| x.len()).sum()
    }

    /// Returns the size of chunk proof.
    pub fn chunk_path_len(&self) -> usize {
        self.chunk_proof.size()
    }

    /// Returns true if the proof is valid given `poa_config`.
    pub fn is_valid(&self, poa_config: &PoaConfiguration) -> bool {
        let PoaConfiguration {
            max_depth,
            max_tx_path,
            max_chunk_path,
        } = poa_config;

        self.depth > 0
            && self.depth <= *max_depth
            && self.tx_path_len() <= *max_tx_path as usize
            && self.chunk_path_len() <= *max_chunk_path as usize
    }
}

/// This struct represents the outcome of creating the inherent data of [`ProofOfAccess`].
#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum PoaOutcome {
    /// Not required for this block due to the entire weave is empty.
    Skipped,
    /// Failed to create a valid [`ProofOfAccess`] due to the maximum depth limit has been reached.
    MaxDepthReached(u32),
    /// Generate a [`ProofOfAccess`] successfully.
    ///
    /// Each block contains a justification of poa as long as the weave
    /// size is not zero and will be verified on block import.
    Justification(ProofOfAccess),
}

impl PoaOutcome {
    /// Returns true if the poa inherent must be included in the block.
    pub fn require_inherent(&self) -> bool {
        matches!(self, Self::Justification(..))
    }
}

const MAX_DEPTH: u32 = 1_000;
const MAX_TX_PATH: u32 = 256 * 1024;
const MAX_CHUNK_PATH: u32 = 256 * 1024;

/// Configuration of the PoA consensus engine.
#[derive(Clone, Eq, PartialEq, Encode, Decode, MaxEncodedLen)]
pub struct PoaConfiguration {
    /// The maximum depth of attempting to generate a valid [`ProofOfAccess`].
    pub max_depth: u32,
    /// Maximum byte size of tx merkle path.
    pub max_tx_path: u32,
    /// Maximum byte size of chunk merkle path.
    pub max_chunk_path: u32,
}

impl Default for PoaConfiguration {
    fn default() -> Self {
        Self {
            max_depth: MAX_DEPTH,
            max_tx_path: MAX_TX_PATH,
            max_chunk_path: MAX_CHUNK_PATH,
        }
    }
}

impl PoaConfiguration {
    /// Returns true if all the sanity checks are passed.
    pub fn check_sanity(&self) -> bool {
        // TODO:
        // 1. upper limit check?
        // 2. more accurate check for the proof since the size of merkle proof has a lower bound?
        self.max_depth > 0 && self.max_tx_path > 0 && self.max_chunk_path > 0
    }
}

impl sp_std::fmt::Debug for PoaConfiguration {
    #[cfg(feature = "std")]
    fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        f.debug_struct("PoaConfiguration")
            .field("max_depth", &self.max_depth)
            .field("max_tx_path", &self.max_tx_path)
            .field("max_chunk_path", &self.max_chunk_path)
            .finish()
    }

    #[cfg(not(feature = "std"))]
    fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        f.write_str("<wasm:stripped>")
    }
}
