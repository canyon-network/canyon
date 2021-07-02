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
//!
//! TODO: verify PoA on block import.

use codec::{Decode, Encode};
use thiserror::Error;

use sp_blockchain::HeaderBackend;
use sp_runtime::{
    generic::BlockId,
    traits::{Block as BlockT, DigestItemFor, Extrinsic, Header as HeaderT},
};

use sc_client_api::BlockBackend;

use canyon_primitives::{DataIndex, Depth, ExtrinsicIndex};
use cp_permastore::{TransactionDataBackend as TransactionDataBackendT, CHUNK_SIZE, POA_ENGINE_ID};

mod chunk_proof;
mod tx_proof;

use self::chunk_proof::ChunkProof;

type TxProof = Vec<Vec<u8>>;

/// The maximum depth of attempting to generate a valid PoA.
///
/// TODO: make it configurable in Runtime?
pub const MAX_DEPTH: u32 = 100;

type Randomness = Vec<u8>;

#[derive(Error, Debug)]
pub enum Error<Block: BlockT> {
    #[error("codec error")]
    Codec(#[from] codec::Error),
    #[error("blockchain error")]
    BlockchainError(#[from] sp_blockchain::Error),
    #[error("block {0} not found")]
    BlockNotFound(BlockId<Block>),
    #[error("block header {0} not found")]
    HeaderNotFound(BlockId<Block>),
    #[error("the chunk in recall tx not found")]
    InvalidChunk,
    #[error("weave size not found in the header digests")]
    EmptyWeaveSize,
    #[error("the maximum allowed depth {0} reached")]
    MaxDepthReached(Depth),
    #[error("unknown poa error")]
    Unknown,
}

/// Type for proving the data access.
#[derive(Debug, Clone, Encode, Decode)]
pub struct Poa {
    ///
    pub depth: Depth,
    ///
    pub tx_path: TxProof,
    ///
    pub chunk_proof: ChunkProof,
}

/// Applies the hashing on `seed` for `n` times
fn multihash(seed: Randomness, n: Depth) -> [u8; 32] {
    assert!(n > 0);
    let mut r = sp_io::hashing::blake2_256(&seed);
    for _ in 1..n {
        r = sp_io::hashing::blake2_256(&r);
    }
    r
}

fn make_bytes(h: [u8; 32]) -> [u8; 8] {
    let mut res = [0u8; 8];
    res.copy_from_slice(&h);
    res
}

/// Returns the position of recall byte in the entire weave.
fn calculate_challenge_byte(seed: Randomness, weave_size: DataIndex, depth: Depth) -> DataIndex {
    assert!(weave_size > 0, "weave size can not be 0");
    DataIndex::from_le_bytes(make_bytes(multihash(seed, depth))) % weave_size
}

fn binary_search<T: Copy>(target: DataIndex, ordered_list: &[(T, DataIndex)]) -> (T, DataIndex) {
    match ordered_list.binary_search_by_key(&target, |&(_, weave_size)| weave_size) {
        Ok(i) => ordered_list[i],
        Err(i) => ordered_list[i],
    }
}

/// Returns a tuple of (extrinsic_index, absolute_data_index) of extrinsic in which the recall byte is located.
fn find_recall_tx(
    recall_byte: DataIndex,
    sized_extrinsics: &[(ExtrinsicIndex, DataIndex)],
) -> (ExtrinsicIndex, DataIndex) {
    binary_search(recall_byte, sized_extrinsics)
    // let (extrinsic_index, data_index) = binary_search(recall_byte, sized_extrinsics);
    // (extrinsic_index as ExtrinsicIndex, data_index)
}

fn extract_weave_size<Block: BlockT>(header: &Block::Header) -> Result<DataIndex, Error<Block>> {
    let opaque_weave_size = header.digest().logs.iter().find_map(|log| {
        if let DigestItemFor::<Block>::Consensus(POA_ENGINE_ID, opaque_data) = log {
            Some(opaque_data)
        } else {
            None
        }
    });

    match opaque_weave_size {
        Some(weave_size) => Decode::decode(&mut weave_size.as_slice()).map_err(Error::Codec),
        None => {
            Ok(Default::default())

            // FIXME: weave size should only be zero for genesis?
            // Err(Error::EmptyWeaveSize),
        }
    }
}

fn fetch_block<Block: BlockT, Client: BlockBackend<Block>>(
    id: BlockId<Block>,
    client: &Client,
) -> Result<(Block::Header, Vec<Block::Extrinsic>), Error<Block>> {
    Ok(client
        .block(&id)?
        .ok_or_else(|| Error::BlockNotFound(id))?
        .block
        .deconstruct())
}

fn fetch_header<Block: BlockT, Client: HeaderBackend<Block>>(
    id: BlockId<Block>,
    client: &Client,
) -> Result<Block::Header, Error<Block>> {
    client.header(id)?.ok_or_else(|| Error::HeaderNotFound(id))
}

/// Returns the block number of recall block.
fn find_recall_block<Block: BlockT>(recall_byte: DataIndex) -> BlockId<Block> {
    todo!("find recall block number")
}

/// Constructs a valid PoA.
pub fn construct_poa<
    Block: BlockT + 'static,
    Client: BlockBackend<Block> + HeaderBackend<Block> + 'static,
    TransactionDataBackend: TransactionDataBackendT<Block>,
>(
    client: &Client,
    parent: Block::Hash,
    transaction_data_backend: TransactionDataBackend,
) -> Result<Option<Poa>, Error<Block>> {
    let chain_head = fetch_header(BlockId::Hash(parent), client)?;

    let weave_size = extract_weave_size::<Block>(&chain_head)?;

    for depth in 1..=MAX_DEPTH {
        // Genesis block?
        if weave_size == 0 {
            return Ok(None);
        }

        let recall_byte = calculate_challenge_byte(chain_head.encode(), weave_size, depth);
        let recall_block_id = find_recall_block(recall_byte);

        let (header, extrinsics) = fetch_block(recall_block_id, client)?;

        let recall_parent_block_id = BlockId::Hash(*header.parent_hash());
        let recall_parent_header = fetch_header(recall_parent_block_id, client)?;

        let weave_base = extract_weave_size::<Block>(&recall_parent_header)?;

        let mut sized_extrinsics = Vec::with_capacity(extrinsics.len());

        let mut acc = 0u64;
        for (index, extrinsic) in extrinsics.iter().enumerate() {
            let tx_size = extrinsic.data_size();
            if tx_size > 0 {
                sized_extrinsics.push((index as ExtrinsicIndex, weave_base + acc + tx_size));
                acc += tx_size;
            }
        }

        // No data store transactions in this block.
        if sized_extrinsics.is_empty() {
            continue;
        }

        let (recall_extrinsic_index, recall_tx_data_base) =
            find_recall_tx(recall_byte, &sized_extrinsics);

        // Continue if the recall tx has been forgotten as the forgot
        // txs can not participate in the consensus.
        //
        // FIXME: handle the data oblivion
        // if todo!("recall_tx has been forgotten via runtime api") {
        // continue;
        // }

        if let Ok(Some(tx_data)) = transaction_data_backend
            .transaction_data(recall_block_id, recall_extrinsic_index as u32)
        {
            let chunk_ids = chunk_proof::chunk_ids(tx_data);

            let chunk_offset = recall_byte - recall_tx_data_base;
            let recall_chunk_index = chunk_offset / CHUNK_SIZE;

            let recall_chunk_id = chunk_ids[recall_chunk_index as usize].clone();

            // Find the chunk

            // Construct PoA proof.

            // If find one solution, return directly.
        } else {
            log::error!(
                "transaction data not found given block {} and extrinsic index {}",
                recall_block_id,
                recall_extrinsic_index
            );
        }
    }

    Err(Error::MaxDepthReached(MAX_DEPTH))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct GlobalBlockIndex(Vec<(BlockNumber, DataIndex)>);

    impl GlobalBlockIndex {
        pub fn find_challenge_block(&self, recall_byte: DataIndex) -> BlockNumber {
            binary_search(recall_byte, &self.0).0
        }
    }

    #[test]
    fn test_find_challenge_block() {
        let global_index = GlobalBlockIndex(vec![(0, 10), (3, 15), (5, 20), (6, 30), (7, 32)]);

        assert_eq!(0, global_index.find_callenge_block(5));
        assert_eq!(3, global_index.find_callenge_block(15));
        assert_eq!(5, global_index.find_callenge_block(16));
        assert_eq!(6, global_index.find_callenge_block(29));
        assert_eq!(7, global_index.find_callenge_block(31));
    }
}
