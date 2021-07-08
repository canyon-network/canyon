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

use sp_runtime::ConsensusEngineId;
use sp_std::vec::Vec;

pub const POA_ENGINE_ID: ConsensusEngineId = *b"poa_";

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
    /// Returns the proof size in bytes.
    pub fn size(&self) -> usize {
        self.proof.iter().map(|p| p.len()).sum()
    }
}

/// Type for proving the historical data access.
#[derive(Debug, Clone, Encode, Decode)]
pub struct ProofOfAccess {
    /// poa depth.
    pub depth: u32,
    /// merkle path of recall tx.
    pub tx_path: Vec<Vec<u8>>,
    /// data chunk proof.
    pub chunk_proof: ChunkProof,
}
