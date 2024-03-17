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

pub mod error;

use self::error::Error;
use jsonrpsee::proc_macros::rpc;
use sc_rpc_api::author::hash::ExtrinsicOrHash;
use sp_core::{Bytes, H256};

/// Canyon perma storage RPC API.
#[rpc(client, server)]
pub trait PermastoreApi<Hash, BlockHash> {
    /// Sepecialized `submit_extrinsic` for submitting the store extrinsic and transaction data.
    #[method(name = "permastore_submitExtrinsic")]
    async fn submit_extrinsic(&self, ext: Bytes, data: Bytes) -> Result<Hash, Error>;

    /// Sepecialized `remove_extrinsic` for removing the extrinsic and data if any.
    #[method(name = "permastore_removeExtrinsic")]
    fn remove_extrinsic(
        &self,
        bytes_or_hash: Vec<ExtrinsicOrHash<Hash>>,
    ) -> Result<Vec<Hash>, Error>;

    /// Remove the data of a transaction.
    #[method(name = "permastore_removeData")]
    fn remove_data(&self, chunk_root: BlockHash) -> Result<bool, Error>;

    /// Submit the whole data of a transaction.
    #[method(name = "permastore_submit")]
    fn submit(&self, value: Bytes) -> Result<H256, Error>;

    /// Fetch storage under given key.
    #[method(name = "permastore_retrieve")]
    fn retrieve(&self, key: Bytes) -> Result<Option<Bytes>, Error>;
}
