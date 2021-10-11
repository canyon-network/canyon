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

#[cfg(test)]
mod tests;

use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;

use futures::future::FutureExt;
use parking_lot::RwLock;

use sc_rpc_api::{
    author::{error::FutureResult, hash::ExtrinsicOrHash, AuthorApi},
    DenyUnsafe,
};
use sc_transaction_pool_api::{TransactionPool, TxHash};

use sp_core::{Bytes, Encode, H256};
use sp_runtime::traits::{BlakeTwo256, Block as BlockT, Hash};

use cc_rpc_api::permastore::{
    error::{Error, InvalidCount, Result},
    PermastoreApi,
};
use cp_permastore::{PermaStorage, CHUNK_SIZE};

#[derive(Debug)]
pub struct Permastore<T, P, A, B> {
    /// Permanent data storage.
    storage: Arc<RwLock<T>>,
    /// Transaction pool.
    pool: Arc<P>,
    /// Authoring api.
    author: A,
    /// Whether to deny unsafe calls
    ///
    /// TODO: since this is a pretty dangerous operation we might
    /// need a more restricted way to prevent from the risks.
    deny_unsafe: DenyUnsafe,
    /// Block.
    phatom: PhantomData<B>,
}

impl<T, P, A, B> Permastore<T, P, A, B> {
    pub fn new(storage: T, pool: Arc<P>, author: A, deny_unsafe: DenyUnsafe) -> Self {
        Self {
            storage: Arc::new(RwLock::new(storage)),
            pool,
            author,
            deny_unsafe,
            phatom: PhantomData::<B>,
        }
    }
}

/// Maximum byte size of uploading transaction data directly. 10MiB
const MAX_UPLOAD_DATA_SIZE: u32 = 10 * 1024 * 1024;

/// Maximum byte size of downloading transaction data directly. 12MiB
const MAX_DOWNLOAD_DATA_SIZE: u32 = 12 * 1024 * 1024;

impl<T, P, A, B> PermastoreApi<TxHash<P>, <B as BlockT>::Hash> for Permastore<T, P, A, B>
where
    T: PermaStorage + 'static,
    P: TransactionPool + Send + Sync + 'static,
    B: BlockT,
    A: AuthorApi<TxHash<P>, <B as BlockT>::Hash>,
{
    fn submit_extrinsic(&self, ext: Bytes, data: Bytes) -> FutureResult<TxHash<P>> {
        if let Err(e) = self.submit(data) {
            return async move { Err(sc_rpc_api::author::error::Error::Client(Box::new(e))) }
                .boxed();
        }
        self.author.submit_extrinsic(ext)
    }

    fn remove_extrinsic(
        &self,
        bytes_or_hash: Vec<ExtrinsicOrHash<TxHash<P>>>,
    ) -> Result<Vec<TxHash<P>>> {
        // FIXME: remove the transaction data directly or later?
        Ok(self.author.remove_extrinsic(bytes_or_hash)?)
    }

    fn remove_data(&self, chunk_root: <B as BlockT>::Hash) -> Result<bool> {
        self.deny_unsafe.check_if_safe()?;

        self.storage.write().remove(chunk_root.encode().as_slice());

        Ok(true)
    }

    // Can this be an attack as anyone can submit arbitrary data to the node?
    // TODO: add tests for submit and retrieve?
    fn submit(&self, value: Bytes) -> Result<H256> {
        let data_size = value.deref().len() as u32;
        if data_size > MAX_UPLOAD_DATA_SIZE {
            return Err(Error::DataTooLarge(InvalidCount::new(
                data_size,
                MAX_UPLOAD_DATA_SIZE,
            )));
        }

        let chunks = value
            .0
            .chunks(CHUNK_SIZE as usize)
            .map(|c| BlakeTwo256::hash(c).encode())
            .collect();

        let chunk_root = BlakeTwo256::ordered_trie_root(chunks);

        let key = chunk_root.encode();

        log::debug!(
            target: "rpc::permastore",
            "Submitted chunk_root: {:?}, stored key: {:?}",
            chunk_root, key,
        );

        // TODO: verify chunk_root matches the submitted data.
        self.storage.write().submit(key.as_slice(), &*value);

        Ok(chunk_root)
    }

    fn retrieve(&self, chunk_root: H256) -> Result<Option<Bytes>> {
        let key = chunk_root.encode();
        log::debug!(target: "rpc::permastore", "Retrieving chunk_root: {:?}", chunk_root);
        if let Some(value) = self.storage.read().retrieve(&*key) {
            log::debug!(target: "rpc::permastore", "Retrieving value: {:?}", value);
            let data_size = value.len() as u32;
            if data_size > MAX_DOWNLOAD_DATA_SIZE {
                return Err(Error::DataTooLarge(InvalidCount::new(
                    data_size,
                    MAX_DOWNLOAD_DATA_SIZE,
                )));
            }
            Ok(Some(value.into()))
        } else {
            log::debug!(target: "rpc::permastore", "Error! no value for: {:?}", chunk_root);
            Ok(None)
        }
    }
}
