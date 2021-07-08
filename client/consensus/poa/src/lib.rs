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
//! TODO: verify PoA stored in the block header.

use std::sync::Arc;

use codec::Encode;
use thiserror::Error;

use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{
    generic::BlockId,
    traits::{Block as BlockT, Header as HeaderT, NumberFor},
};

use sc_client_api::BlockBackend;

use canyon_primitives::{DataIndex, Depth, ExtrinsicIndex};
use cc_client_db::TransactionDataBackend as TransactionDataBackendT;
use cp_consensus_poa::PoaOutcome;
use cp_permastore::{PermastoreApi, CHUNK_SIZE};

mod chunk_proof;
mod inherent;
mod tx_proof;

pub use self::chunk_proof::{verify_chunk_proof, ChunkProofBuilder, ChunkProofVerifier};
pub use self::inherent::InherentDataProvider;
pub use self::tx_proof::{build_extrinsic_proof, verify_extrinsic_proof};
pub use cp_consensus_poa::{ChunkProof, ProofOfAccess};

const MIN_DEPTH: u32 = 1;

/// The maximum depth of attempting to generate a valid PoA.
///
/// TODO: make it configurable in Runtime?
pub const MAX_DEPTH: u32 = 1_000;

/// Maximum byte size of transaction merkle path.
pub const MAX_TX_PATH: u32 = 256 * 1024;

/// Maximum byte size of chunk merkle path.
pub const MAX_CHUNK_PATH: u32 = 256 * 1024;

type Randomness = Vec<u8>;

#[derive(Error, Debug)]
pub enum Error<Block: BlockT> {
    #[error("codec error")]
    Codec(#[from] codec::Error),
    #[error("blockchain error")]
    BlockchainError(#[from] sp_blockchain::Error),
    #[error(transparent)]
    ApiError(#[from] sp_api::ApiError),
    #[error("block {0} not found")]
    BlockNotFound(BlockId<Block>),
    #[error("recall block not found given the recall byte {0}")]
    RecallBlockNotFound(DataIndex),
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

/// Applies the hashing on `seed` for `n` times
fn multihash(seed: Randomness, n: Depth) -> [u8; 32] {
    assert!(n > 0, "n can not be 0 when calculating multihash");
    let mut r = sp_io::hashing::blake2_256(&seed);
    for _ in 1..n {
        r = sp_io::hashing::blake2_256(&r);
    }
    r
}

fn make_bytes(h: [u8; 32]) -> [u8; 8] {
    let mut res = [0u8; 8];
    res.copy_from_slice(&h[..8]);
    res
}

/// Returns the position of recall byte in the entire weave.
fn calculate_challenge_byte(seed: Randomness, weave_size: DataIndex, depth: Depth) -> DataIndex {
    assert!(
        weave_size > 0,
        "weave size can not be 0 when calculating the recall byte"
    );
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
    log::debug!(
        target: "poa",
        "finding recall tx, recall_byte: {}, sized_extrinsics: {:?}",
        recall_byte, sized_extrinsics
    );
    binary_search(recall_byte, sized_extrinsics)
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

/// Returns the block number of recall block.
fn find_recall_block<Block, RA>(
    at: BlockId<Block>,
    recall_byte: DataIndex,
    runtime_api: Arc<RA>,
) -> Result<NumberFor<Block>, Error<Block>>
where
    Block: BlockT,
    RA: ProvideRuntimeApi<Block> + Send + Sync,
    RA::Api: PermastoreApi<Block, NumberFor<Block>, u32, Block::Hash>,
{
    log::debug!(
        target: "poa",
        "Calling into runtime to find the recall block, at: {:?}, recall_byte: {:?}",
        at, recall_byte,
    );
    runtime_api
        .runtime_api()
        .find_recall_block(&at, recall_byte)?
        .ok_or(Error::RecallBlockNotFound(recall_byte))
}

/// Constructs a valid Proof of Access.
pub fn construct_poa<Block, Client, TransactionDataBackend, RA>(
    client: &Client,
    parent: Block::Hash,
    transaction_data_backend: TransactionDataBackend,
    runtime_api: Arc<RA>,
) -> Result<PoaOutcome, Error<Block>>
where
    Block: BlockT<Hash = canyon_primitives::Hash> + 'static,
    Client: BlockBackend<Block> + HeaderBackend<Block> + 'static,
    TransactionDataBackend: TransactionDataBackendT<Block>,
    RA: ProvideRuntimeApi<Block> + Send + Sync,
    RA::Api: cp_permastore::PermastoreApi<Block, NumberFor<Block>, u32, Block::Hash>,
{
    let parent_id = BlockId::Hash(parent);

    let weave_size = runtime_api.runtime_api().weave_size(&parent_id)?;

    if weave_size == 0 {
        log::debug!(target: "poa", "Skip constructing poa as the weave size is 0");
        return Ok(PoaOutcome::Skipped);
    }

    for depth in MIN_DEPTH..=MAX_DEPTH {
        log::debug!(target: "poa", "Attempting to generate poa at depth: {}", depth);
        let recall_byte = calculate_challenge_byte(parent.encode(), weave_size, depth);
        let recall_block_number = find_recall_block(parent_id, recall_byte, runtime_api.clone())?;
        let recall_block_id = BlockId::number(recall_block_number);
        log::debug!(
            target: "poa", "Recall block: {} was found given the recall byte: {}",
            recall_block_id,
            recall_byte,
        );

        let (header, extrinsics) = fetch_block(recall_block_id, client)?;

        let recall_parent_block_id = BlockId::Hash(*header.parent_hash());

        let recall_block_weave_base = runtime_api
            .runtime_api()
            .weave_size(&recall_parent_block_id)?;

        let mut sized_extrinsics = Vec::with_capacity(extrinsics.len());

        let mut acc = 0u64;
        for (index, _extrinsic) in extrinsics.iter().enumerate() {
            let tx_size = runtime_api.runtime_api().data_size(
                &recall_block_id,
                recall_block_number,
                index as ExtrinsicIndex,
            )? as u64;
            if tx_size > 0 {
                sized_extrinsics.push((
                    index as ExtrinsicIndex,
                    recall_block_weave_base + acc + tx_size,
                ));
                acc += tx_size;
            }
        }

        log::debug!(
            target: "poa",
            "Sized extrinsics in the recall block: {:?}",
            sized_extrinsics,
        );

        // No data store transactions in this block.
        if sized_extrinsics.is_empty() {
            continue;
        }

        let (recall_extrinsic_index, _recall_block_data_ceil) =
            find_recall_tx(recall_byte, &sized_extrinsics);

        // Continue if the recall tx has been forgotten as the forgot
        // txs can not participate in the consensus.
        //
        // FIXME: handle the data oblivion
        // if todo!("recall_tx has been forgotten via runtime api") {
        // continue;
        // }

        let data_result = transaction_data_backend
            .transaction_data(recall_block_id, recall_extrinsic_index as u32);

        match data_result {
            Ok(Some(tx_data)) => {
                let transaction_data_offset = match recall_byte.checked_sub(recall_block_weave_base)
                {
                    Some(offset) => offset,
                    None => panic!(
                        "Underflow happens! recall_byte: {}, recall_block_weave_base: {}",
                        recall_byte, recall_block_weave_base
                    ),
                };

                if let Ok(chunk_proof) =
                    ChunkProofBuilder::new(tx_data, CHUNK_SIZE, transaction_data_offset as u32)
                        .build()
                {
                    if chunk_proof.size() > MAX_CHUNK_PATH as usize {
                        log::debug!(
                            target: "poa",
                            "Dropping the chunk proof as it's too large ({} > {})",
                            chunk_proof.size(),
                            MAX_CHUNK_PATH,
                        );
                        continue;
                    }
                    if let Ok(tx_proof) = build_extrinsic_proof::<Block>(
                        recall_extrinsic_index,
                        *header.extrinsics_root(),
                        extrinsics,
                    ) {
                        let tx_path_size: usize = tx_proof.iter().map(|t| t.len()).sum();
                        if tx_path_size > MAX_TX_PATH as usize {
                            log::debug!(
                                target: "poa",
                                "Dropping the tx proof as it's too large ({} > {})",
                                tx_path_size,
                                MAX_TX_PATH,
                            );
                            continue;
                        }
                        let poa = ProofOfAccess {
                            depth,
                            tx_path: tx_proof,
                            chunk_proof,
                        };
                        log::debug!(target: "poa", "Generated poa proof: {:?}", poa);
                        return Ok(PoaOutcome::Justification(poa));
                    }
                }
            }
            Ok(None) => {
                log::warn!(
                    target: "poa",
                    "Transaction data not found given block {} and extrinsic index {}",
                    recall_block_id,
                    recall_extrinsic_index
                );
            }
            Err(e) => {
                log::error!(
                    target: "poa",
                    "Error occurred when retrieving the transaction data: {:?}",
                    e,
                );
            }
        }
    }

    log::warn!(target: "poa", "Reaching the max depth: {}", MAX_DEPTH);
    Ok(PoaOutcome::MaxDepthReached)
}
