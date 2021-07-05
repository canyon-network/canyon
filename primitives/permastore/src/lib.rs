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

use sp_runtime::{
    generic::BlockId,
    traits::{Block as BlockT, Header as HeaderT},
};
use sp_std::vec::Vec;

pub const POA_ENGINE_ID: [u8; 4] = *b"poa_";

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
pub trait PermaStorage: Send + Sync + Clone {
    /// Persist a value in storage under given key.
    fn submit(&mut self, key: &[u8], value: &[u8]);

    /// Retrieve a value from storage under given key.
    fn retrieve(&self, key: &[u8]) -> Option<Vec<u8>>;

    /// Checks if the storage exists under given key.
    fn exists(&self, key: &[u8]) -> bool {
        self.retrieve(key).is_some()
    }
}

/// Permanent transaction data backend.
///
/// High level API for accessing the transaction data.
pub trait TransactionDataBackend<Block: BlockT>: PermaStorage + Send + Sync {
    /// Get transaction data. Returns `None` if data is not found.
    fn transaction_data(&self, id: BlockId<Block>, extrinsic_index: u32) -> Option<Vec<u8>>;

    fn chunk_root(
        &self,
        at: Option<BlockId<Block>>,
        block_number: <<Block as BlockT>::Header as HeaderT>::Number,
        extrinsic_index: u32,
    ) -> Option<<<Block as BlockT>::Header as HeaderT>::Hash>;
}

sp_api::decl_runtime_apis! {
    /// The API to query chunk root.
    pub trait PermastoreApi<BlockNumber, ExtrinsicIndex, Hash> where
        BlockNumber: codec::Codec,
        ExtrinsicIndex: codec::Codec,
        Hash: codec::Codec,
    {
        /// Get chunk root given the block number and extrinsic index.
        fn chunk_root(block_number: BlockNumber, extrinsic_index: ExtrinsicIndex) -> Option<Hash>;
    }
}
