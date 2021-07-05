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

//! Permament storage backed by the persistent database.

use std::sync::Arc;

use codec::Encode;

use sc_client_db::{offchain::LocalStorage, Database, DbHash};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{
    generic::BlockId,
    offchain::OffchainStorage,
    traits::{Block as BlockT, Header as HeaderT},
};

use cp_permastore::PermaStorage;
use canyon_primitives::{BlockNumber, Hash};

#[derive(Clone)]
pub struct PermanentStorage<C> {
    offchain_storage: LocalStorage,
    client: C,
}

impl<C> PermanentStorage<C> {
    /// Create permanent storage backed by offchain storage.
    pub fn new(offchain_storage: LocalStorage, client: C) -> Self {
        Self {
            offchain_storage,
            client,
        }
    }
}

impl<C: Send + Sync + Clone> cp_permastore::PermaStorage for PermanentStorage<C> {
    /// key: chunk_root
    /// value: transaction data
    fn submit(&mut self, key: &[u8], value: &[u8]) {
        self.offchain_storage
            .set(sp_offchain::STORAGE_PREFIX, key, value)
    }

    /// key: chunk_root
    fn retrieve(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.offchain_storage.get(sp_offchain::STORAGE_PREFIX, key)
    }
}

impl<Block: BlockT, C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + Clone>
    cp_permastore::TransactionDataBackend<Block> for PermanentStorage<C>
{
    fn transaction_data(&self, block_id: BlockId<Block>, extrinsic_index: u32) -> Option<Vec<u8>> {
        let chunk_root = self.chunk_root(
            None,
            self.client
                .block_number_from_id(&block_id)
                .expect("todo! ")
                .unwrap(),
            extrinsic_index,
        );
        self.retrieve(&chunk_root.encode())
    }

    /// Returns the chunk root at block `block_id` given `block_number` and `extrinsic_index`.
    fn chunk_root(
        &self,
        at: Option<BlockId<Block>>,
        block_number: <<Block as BlockT>::Header as HeaderT>::Number,
        extrinsic_index: u32,
    ) -> Option<<<Block as BlockT>::Header as HeaderT>::Hash> {
        let at = at.unwrap_or_else(|| BlockId::hash(self.client.info().best_hash));
        self.client
            .runtime_api()
            .chunk_root(&at, block_number, extrinsic_index)
    }
}

/// Permanent transaction data backend.
///
/// High level API for accessing the transaction data.
pub trait TransactionDataBackend<Block: BlockT>: Send + Sync {
    /// Get transaction data. Returns `None` if data is not found.
    fn transaction_data(
        &self,
        id: BlockId<Block>,
        extrinsic_index: u32,
    ) -> sp_blockchain::Result<Option<Vec<u8>>>;
}

// (block_id, extrinsic_index) => chunk_root
// chunk_root => transaction_data
// impl<T, Block: BlockT> TransactionDataBackend<Block> for T {
// fn transaction_data(
// &self,
// _id: BlockId<Block>,
// _extrinsic_index: u32,
// ) -> sp_blockchain::Result<Option<Vec<u8>>> {
// todo!("Impl it using the underlying offchain storage")
// }
// }
