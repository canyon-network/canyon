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

/// The identifier for the poa inherent.
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

impl ChunkProof {
    /// Returns the proof size in bytes.
    pub fn size(&self) -> usize {
        self.proof.iter().map(|p| p.len()).sum()
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

/// Outcome of generating the inherent data of [`ProofOfAccess`].
#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub enum PoaOutcome {
    /// Not required for this block.
    Skipped,
    /// Failed to create a valid proof of access due to the max depth limit has been reached.
    MaxDepthReached,
    /// Generate a [`ProofOfAccess`] successfully.
    ///
    /// Each block contains a justification of poa as long as the weave size is not 0
    /// and will be verified on block import.
    Justification(ProofOfAccess),
}

impl PoaOutcome {
    /// Returns true if the poa inherent must be included given the poa outcome.
    pub fn require_inherent(&self) -> bool {
        match self {
            Self::Skipped => false,
            Self::MaxDepthReached => false,
            Self::Justification(_) => true,
        }
    }
}
