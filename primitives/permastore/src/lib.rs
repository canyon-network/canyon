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
#![allow(clippy::too_many_arguments)]

use sp_std::vec::Vec;

/// 256B per chunk.
pub const CHUNK_SIZE: u32 = 256 * 1024;

/// Hasher type for permastore.
#[cfg(feature = "std")]
pub type Hasher = sp_core::Blake2Hasher;

/// Trie layout used for permastore.
#[cfg(feature = "std")]
pub type TrieLayout = sp_trie::Layout<Hasher>;

/// Error type of chunk proof verification.
pub type VerifyError = sp_trie::VerifyError<sp_core::H256, sp_trie::Error>;

/// Low level APIs for manipulating the persistent transaction data storage.
/// No data validation performed.
pub trait PermaStorage: Send + Sync {
    /// Persist a value in storage under given key.
    fn submit(&mut self, key: &[u8], value: &[u8]);

    /// Remove the value under given key.
    fn remove(&mut self, key: &[u8]);

    /// Retrieve a value from storage under given key.
    fn retrieve(&self, key: &[u8]) -> Option<Vec<u8>>;

    /// Checks if the storage exists under given key.
    fn exists(&self, key: &[u8]) -> bool {
        self.retrieve(key).is_some()
    }
}

sp_api::decl_runtime_apis! {
    /// The permastore API.
    pub trait PermastoreApi<BlockNumber, ExtrinsicIndex, Hash> where
        BlockNumber: codec::Codec,
        ExtrinsicIndex: codec::Codec,
        Hash: codec::Codec,
    {
        /// Returns the chunk root given `block_number` and `extrinsic_index`.
        fn chunk_root(block_number: BlockNumber, extrinsic_index: ExtrinsicIndex) -> Option<Hash>;

        /// Returns the number of block in which the recall byte is included.
        fn find_recall_block(recall_byte: u64) -> Option<BlockNumber>;

        /// Returns the size of transaction data given `block_number` and `extrinsic_index`.
        fn data_size(block_number: BlockNumber, extrinsic_index: ExtrinsicIndex) -> u32;

        /// Returns `true` if the proof of access is required for the block.
        fn require_proof_of_access() -> bool;

        /// Returns the size of current block.
        fn block_size() -> u64;

        /// Returns the size of entire weave.
        fn weave_size() -> u64;
    }
}
