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

//! This crate creates the inherent data based on the Proof of Access consensus.

use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block as BlockT;

use sc_client_api::BlockBackend;

use cc_consensus_poa::{construct_poa, Error};
use cp_permastore::TransactionDataBackend as TransactionDataBackendT;

pub struct InherentDataProvider {
    /// Depth
    pub inherent_data: Option<u32>,
}

impl InherentDataProvider {
    /// Creates a new instance of `InherentDataProvider`.
    pub fn create<
        Block: BlockT + 'static,
        Client: BlockBackend<Block> + HeaderBackend<Block> + 'static,
        TransactionDataBackend: TransactionDataBackendT<Block>,
    >(
        client: &Client,
        parent: Block::Hash,
        transaction_data_backend: TransactionDataBackend,
    ) -> Result<Self, Error<Block>> {
        let inherent_data =
            construct_poa(client, parent, transaction_data_backend)?.map(|poa| poa.depth as u32);
        Ok(Self { inherent_data })
    }
}

#[async_trait::async_trait]
impl sp_inherents::InherentDataProvider for InherentDataProvider {
    fn provide_inherent_data(
        &self,
        inherent_data: &mut sp_inherents::InherentData,
    ) -> Result<(), sp_inherents::Error> {
        inherent_data.put_data(
            canyon_primitives::POA_INHERENT_IDENTIFIER,
            &self.inherent_data,
        )
    }

    async fn try_handle_error(
        &self,
        _: &sp_inherents::InherentIdentifier,
        _: &[u8],
    ) -> Option<Result<(), sp_inherents::Error>> {
        // Inherent isn't checked and can not return any error
        None
    }
}
