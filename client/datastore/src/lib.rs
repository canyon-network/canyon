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

#![deny(missing_docs, unused_extern_crates)]

//! This crate provides the feature of persistent storage for the transaction data
//! expected to exist indefinitely.
//!
//! Currently, it is implemented on the top of offchain storage, which is a persistent
//! local storage of each node.

#[cfg(test)]
mod tests;

use std::sync::Arc;

use codec::Encode;

use sc_client_db::offchain::LocalStorage;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{
    generic::BlockId,
    offchain::OffchainStorage,
    traits::{Block as BlockT, NumberFor},
};

use cp_permastore::{PermaStorage, PermastoreApi};

/// Permanent storage backed by offchain storage.
#[derive(Clone)]
pub struct PermanentStorage<C> {
    offchain_storage: LocalStorage,
    client: Arc<C>,
}

impl<C> PermanentStorage<C> {
    /// Creates new perma storage for tests.
    #[cfg(any(feature = "test-helpers", test))]
    pub fn new_test(client: Arc<C>) -> Self {
        Self {
            offchain_storage: LocalStorage::new_test(),
            client,
        }
    }

    /// Creates a new instance of [`PermaStorage`] backed by offchain storage.
    pub fn new(offchain_storage: LocalStorage, client: Arc<C>) -> Self {
        Self {
            offchain_storage,
            client,
        }
    }
}

impl<C> cp_permastore::PermaStorage for PermanentStorage<C>
where
    C: Send + Sync,
{
    /// Sets the value of transaction data given `key`.
    ///
    /// # Arguments
    ///
    /// * `key`: encoded chunk root of transaction data.
    /// * `value`: entire data of a transaction.
    ///
    /// NOTE: the maximum size of served value is 10MiB,
    /// this limit should be enforced by the higher level API.
    fn submit(&mut self, key: &[u8], value: &[u8]) {
        self.offchain_storage
            .set(sp_offchain::STORAGE_PREFIX, key, value)
    }

    /// Returns the entire transaction data given `key`.
    ///
    /// # Arguments
    ///
    /// * `key`: chunk_root of the transaction data.
    fn retrieve(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.offchain_storage.get(sp_offchain::STORAGE_PREFIX, key)
    }

    /// Removes the storage value under given key.
    ///
    /// # Arguments
    ///
    /// * `key`: encoded chunk root of transaction data.
    fn remove(&mut self, key: &[u8]) {
        self.offchain_storage
            .remove(sp_offchain::STORAGE_PREFIX, key)
    }
}

/// Error type for datastore.
#[derive(thiserror::Error, Debug)]
pub enum Error<Block: BlockT> {
    /// Block number not found.
    #[error("Block number not found given block id `{0}`")]
    BlockNumberNotFound(BlockId<Block>),
    /// Chunk root does not exist.
    #[error("Chunk root is None at block: {0}, extrinsic index: {1}")]
    ChunkRootIsNone(NumberFor<Block>, u32),
    /// Blockchain error.
    #[error(transparent)]
    Blockchain(#[from] Box<sp_blockchain::Error>),
    /// Runtime api error.
    #[error(transparent)]
    ApiError(#[from] sp_api::ApiError),
}

/// Backend for storing a map of (block_number, extrinsic_index) to chunk_root.
pub trait ChunkRootBackend<Block: BlockT> {
    /// Returns chunk root given `block_number` and `extrinsic_index`.
    ///
    /// It's fetched from the runtime now.
    fn chunk_root(
        &self,
        at: Option<Block::Hash>,
        block_number: NumberFor<Block>,
        extrinsic_index: u32,
    ) -> Result<Option<Block::Hash>, Error<Block>>;
}

/// Permanent transaction data backend.
///
/// High level API for accessing the transaction data.
pub trait TransactionDataBackend<Block: BlockT>: PermaStorage + ChunkRootBackend<Block> {
    /// Get transaction data. Returns `None` if data is not found.
    fn transaction_data(
        &self,
        block_number: NumberFor<Block>,
        extrinsic_index: u32,
    ) -> Result<Option<Vec<u8>>, Error<Block>>;
}

impl<Block, C> TransactionDataBackend<Block> for PermanentStorage<C>
where
    Block: BlockT,
    C: HeaderBackend<Block> + ProvideRuntimeApi<Block> + Send + Sync,
    C::Api: cp_permastore::PermastoreApi<Block, NumberFor<Block>, u32, Block::Hash>,
{
    fn transaction_data(
        &self,
        block_number: NumberFor<Block>,
        extrinsic_index: u32,
    ) -> Result<Option<Vec<u8>>, Error<Block>> {
        log::debug!(
            target: "datastore",
            "Fetching chunk root of block#{block_number}, extrinsic_index: {extrinsic_index}",
        );

        let chunk_root = self
            .chunk_root(None, block_number, extrinsic_index)?
            .ok_or(Error::ChunkRootIsNone(block_number, extrinsic_index))?;

        let key = chunk_root.encode();

        log::debug!(
            target: "datastore",
            "Fetching the transaction data, chunk root: {chunk_root:?}, database key: {key:?}",
        );

        Ok(self.retrieve(&key))
    }
}

impl<Block, C> ChunkRootBackend<Block> for PermanentStorage<C>
where
    Block: BlockT,
    C: HeaderBackend<Block> + ProvideRuntimeApi<Block> + Send + Sync,
    C::Api: cp_permastore::PermastoreApi<Block, NumberFor<Block>, u32, Block::Hash>,
{
    fn chunk_root(
        &self,
        at: Option<Block::Hash>,
        block_number: NumberFor<Block>,
        extrinsic_index: u32,
    ) -> Result<Option<Block::Hash>, Error<Block>> {
        let at = at.unwrap_or_else(|| self.client.info().best_hash);
        self.client
            .runtime_api()
            .chunk_root(at, block_number, extrinsic_index)
            .map_err(Error::ApiError)
    }
}
