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

use sp_core::Bytes;

use self::error::Result;

pub use self::gen_client::Client as OffchainClient;

/// Canyon perma storage RPC API.
#[rpc]
pub trait PermastoreApi {
    /// Submit the transaction data under given key.
    #[rpc(name = "permastore_submit")]
    fn submit(&self, key: Bytes, value: Bytes) -> Result<()>;

    /// Fetch storage under given key.
    #[rpc(name = "permastore_retrieve")]
    fn retrieve(&self, key: Bytes) -> Result<Option<Bytes>>;
}
