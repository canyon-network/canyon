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

use jsonrpc_derive::rpc;

use sc_rpc_api::author::{error::FutureResult, hash::ExtrinsicOrHash};

use sp_core::{Bytes, H256};

use self::error::Result;

pub use self::gen_client::Client as OffchainClient;

/// Canyon perma storage RPC API.
#[rpc]
pub trait PermastoreApi<Hash, BlockHash> {
    /// Sepecialized `submit_extrinsic` for submitting the store extrinsic and transaction data.
    #[rpc(name = "permastore_submitExtrinsic")]
    fn submit_extrinsic(&self, ext: Bytes, data: Bytes) -> FutureResult<Hash>;

    /// Sepecialized `remove_extrinsic` for removing the extrinsic and data if any.
    #[rpc(name = "permastore_removeExtrinsic")]
    fn remove_extrinsic(&self, bytes_or_hash: Vec<ExtrinsicOrHash<Hash>>) -> Result<Vec<Hash>>;

    /// Remove the data of a transaction.
    #[rpc(name = "permastore_removeData")]
    fn remove_data(&self, chunk_root: BlockHash) -> Result<bool>;

    /// Submit the whole data of a transaction.
    #[rpc(name = "permastore_submit")]
    fn submit(&self, value: Bytes) -> Result<H256>;

    /// Fetch storage under given key.
    #[rpc(name = "permastore_retrieve")]
    fn retrieve(&self, chunk_root: H256) -> Result<Option<Bytes>>;
}
