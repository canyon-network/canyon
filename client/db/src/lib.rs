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
pub struct PermanentStorage<C, RA> {
    offchain_storage: LocalStorage,
    client: Arc<C>,
    runtime_api: Arc<RA>,
}

impl<C, RA> PermanentStorage<C, RA> {
    /// Create permanent storage backed by offchain storage.
    ///
    /// `runtime_api` is used for getting the chunk root from runtime.
    pub fn new(offchain_storage: LocalStorage, client: Arc<C>, runtime_api: Arc<RA>) -> Self {
        Self {
            offchain_storage,
            client,
            runtime_api,
        }
    }
}

impl<C, RA> cp_permastore::PermaStorage for PermanentStorage<C, RA>
where
    C: Send + Sync,
    RA: Send + Sync,
{
    /// Sets the value of transaction data given `key`.
    ///
    /// # Arguments
    ///
    /// * `key`: chunk_root of the transaction data.
    /// * `value`: entire data of transaction.
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
}

#[derive(thiserror::Error, Debug)]
pub enum Error<Block: BlockT> {
    #[error("block number not found given block id `{0}`")]
    BlockNumberNotFound(BlockId<Block>),
    #[error(transparent)]
    Blockchain(#[from] sp_blockchain::Error),
    #[error(transparent)]
    ApiError(#[from] sp_api::ApiError),
}

/// Backend for storing a map of (block_number, extrinsic_index) to chunk_root.
pub trait ChunkRootBackend<Block: BlockT> {
    /// Returns chunk root given `block_number` and `extrinsic_index`.
    ///
    /// Usually fetched from the runtime.
    fn chunk_root(
        &self,
        at: Option<BlockId<Block>>,
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
        id: BlockId<Block>,
        extrinsic_index: u32,
    ) -> Result<Option<Vec<u8>>, Error<Block>>;
}

impl<Block, C, RA> TransactionDataBackend<Block> for PermanentStorage<C, RA>
where
    Block: BlockT,
    C: HeaderBackend<Block> + Send + Sync,
    RA: ProvideRuntimeApi<Block> + Send + Sync,
    RA::Api: cp_permastore::PermastoreApi<Block, NumberFor<Block>, u32, Block::Hash>,
{
    fn transaction_data(
        &self,
        block_id: BlockId<Block>,
        extrinsic_index: u32,
    ) -> Result<Option<Vec<u8>>, Error<Block>> {
        let chunk_root = self.chunk_root(
            None,
            self.client
                .block_number_from_id(&block_id)?
                .ok_or(Error::BlockNumberNotFound(block_id))?,
            extrinsic_index,
        )?;
        Ok(self.retrieve(&chunk_root.encode()))
    }
}

impl<Block, C, RA> ChunkRootBackend<Block> for PermanentStorage<C, RA>
where
    Block: BlockT,
    C: HeaderBackend<Block> + Send + Sync,
    RA: ProvideRuntimeApi<Block> + Send + Sync,
    RA::Api: cp_permastore::PermastoreApi<Block, NumberFor<Block>, u32, Block::Hash>,
{
    fn chunk_root(
        &self,
        at: Option<BlockId<Block>>,
        block_number: NumberFor<Block>,
        extrinsic_index: u32,
    ) -> Result<Option<Block::Hash>, Error<Block>> {
        let at = at.unwrap_or_else(|| BlockId::hash(self.client.info().best_hash));
        self.runtime_api
            .runtime_api()
            .chunk_root(&at, block_number, extrinsic_index)
            .map_err(|e| Error::ApiError(e))
    }
}
