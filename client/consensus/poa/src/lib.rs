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

//! # Proof of Access consensus
//!
//! ## Introduction
//!
//! Proof of Access is a kind of lightweight storage consensus initially
//! adopted by [Arweave](https://arweave.org). In arweave, PoA serves as
//! an enhancement of Proof of Work in which the entire recall block data
//! is included in the material to be hashed for input to the proof of work.
//!
//! Requiring [`ProofOfAccess`] incentivises storage as miners need
//! access to random blocks from the blockweave's history in order
//! to mine new blocks and receive mining rewards.
//!
//! ## Overview
//!
//! The general workflow of PoA is described briefly below:
//!
//! 1. Pick a random byte from the whole network storage (BlockWeave).
//!     - The block weave can be seen as an ever growing gigantic array.
//!     - Currently, the randome byte is determined by hashing
//!       the parent header hash for N times(see [`calculate_challenge_byte`]),
//!       which will be replaced with another hashing strategy in SPoRA.
//!
//! 2. Locate the extrinsic in which the random byte is included.
//!
//! 3. Check if the data of extrinsic located in Step 2 exists in
//!    the local storage.
//!
//!     - If the data does exist locally, create the two merkle proofs
//!       of extrinsic and data chunks respectively.
//!     - If not, repeat from Step 1 by choosing another random byte.
//!
//! ## Usage
//!
//! Normally, PoA needs to be used with other consensus algorithem like
//! PoW or PoS together as it's not typically designed for solving the
//! problem of selecting one from a set of validators to author next block
//! in an unpredictable or fair way. In another word, PoA is not intended
//! for resolving the leader election problem, and is usually exploited
//! as a precondition for PoW or PoS in order to encourage the miners to
//! store more data locally.
//!
//! This crate implements the core algorithem of Proof of Access in
//! [`construct_poa`] and provides the inherent data provider via
//! [`PoaInherentDataProvider`]. [`PurePoaBlockImport`] implements the
//! `BlockImport` trait, thus can be wrapped in another block importer.
//!
//! To use this engine, you can create an inhehrent extrinsic using the
//! data provided by [`PoaInherentDataProvider`] in a pallet. Furthermore,
//! you need to wrap the [`PurePoaBlockImport`] into your existing block
//! import pipeline. Refer to the [Substrate docs][1] for more information
//! about creating a nested `BlockImport`.
//!
//! [1]: https://substrate.dev/docs/en/knowledgebase/advanced/block-import

#![deny(missing_docs, unused_extern_crates)]

use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

use codec::{Decode, Encode};
use thiserror::Error;

use sc_client_api::{backend::AuxStore, BlockBackend, BlockOf};
use sc_consensus::{BlockCheckParams, BlockImport, BlockImportParams, ImportResult};
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder as BlockBuilderApi;
use sp_blockchain::{well_known_cache_keys::Id as CacheKeyId, HeaderBackend, ProvideCache};
use sp_consensus::{Error as ConsensusError, SelectChain};
use sp_runtime::{
    generic::BlockId,
    traits::{Block as BlockT, Header as HeaderT, NumberFor},
    DigestItem,
};

use canyon_primitives::{DataIndex, Depth, ExtrinsicIndex};
use cc_datastore::TransactionDataBackend as TransactionDataBackendT;
use cp_permastore::{PermastoreApi, CHUNK_SIZE};
use cp_poa::PoaApi;

#[cfg(test)]
mod babe_tests;
mod chunk_proof;
mod inherent;
#[cfg(test)]
mod tests;
mod trie;
mod tx_proof;

pub use self::chunk_proof::{verify_chunk_proof, ChunkProofBuilder, ChunkProofVerifier};
pub use self::inherent::PoaInherentDataProvider;
pub use self::tx_proof::{build_extrinsic_proof, verify_extrinsic_proof, TxProofVerifier};

// Re-exports of the primitives of poa consensus.
pub use cp_consensus_poa::{
    ChunkProof, PoaConfiguration, PoaOutcome, PoaValidityError, ProofOfAccess, POA_ENGINE_ID,
};

/// Minimum depth of PoA.
const MIN_DEPTH: u32 = 1;

type Randomness = Vec<u8>;

/// Error type for poa consensus.
#[derive(Error, Debug)]
pub enum Error<Block: BlockT> {
    /// No PoA seal in the header.
    #[error("Header {0:?} has no PoA digest")]
    NoDigest(Block::Hash),
    /// Multiple PoA seals were found in the header.
    #[error("Header {0:?} has multiple PoA digests")]
    MultipleDigests(Block::Hash),
    /// Client error.
    #[error("Client error: {0}")]
    Client(sp_blockchain::Error),
    /// Codec error.
    #[error("Codec error: {0}")]
    Codec(#[from] codec::Error),
    /// Blockchain error.
    #[error("Blockchain error: {0}")]
    BlockchainError(#[from] sp_blockchain::Error),
    /// Invalid ProofOfAccess.
    #[error("Invalid ProofOfAccess: {0:?}")]
    InvalidPoa(PoaValidityError),
    /// Failed to verify the merkle proof.
    #[error("VerifyError error: {0:?}")]
    VerifyFailed(#[from] cp_permastore::VerifyError),
    /// Runtime api error.
    #[error(transparent)]
    ApiError(#[from] sp_api::ApiError),
    /// Chunk root not found.
    #[error("Chunk root not found for the recall extrinsic {0}#{1}")]
    ChunkRootNotFound(BlockId<Block>, ExtrinsicIndex),
    /// Block not found.
    #[error("Block {0} not found")]
    BlockNotFound(BlockId<Block>),
    /// Recall block not found.
    #[error("Recall block not found given the recall byte {0}")]
    RecallBlockNotFound(DataIndex),
    /// Recall extrinsic not found.
    #[error("Recall extrinsic index not found given the recall byte {0}")]
    RecallExtrinsicNotFound(DataIndex),
    /// Maxinum depth reached.
    #[error("Reaching the maximum allowed depth {0}")]
    MaxDepthReached(Depth),
}

impl<B: BlockT> From<Error<B>> for ConsensusError {
    fn from(error: Error<B>) -> Self {
        Self::ClientImport(error.to_string())
    }
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
///
/// TODO: SPoRA
pub fn calculate_challenge_byte(
    seed: Randomness,
    weave_size: DataIndex,
    depth: Depth,
) -> DataIndex {
    assert!(
        weave_size > 0,
        "weave size can not be 0 when calculating the recall byte"
    );
    DataIndex::from_le_bytes(make_bytes(multihash(seed, depth))) % weave_size
}

/// Returns a tuple of (extrinsic_index, absolute_data_index)
/// of extrinsic in which `recall_byte` is located.
fn find_recall_tx(
    recall_byte: DataIndex,
    sized_extrinsics: &[(ExtrinsicIndex, DataIndex)],
) -> (ExtrinsicIndex, DataIndex) {
    log::trace!(
        target: "poa",
        "Locating the position of recall tx, recall_byte: {}, sized_extrinsics: {:?}",
        recall_byte, sized_extrinsics
    );
    match sized_extrinsics.binary_search_by_key(&recall_byte, |&(_, weave_size)| weave_size) {
        Ok(i) => sized_extrinsics[i],
        Err(i) => sized_extrinsics[i],
    }
}

/// All information of recall block that is required to build a [`ProofOfAccess`].
#[derive(Debug, Clone)]
pub struct RecallInfo<B: BlockT> {
    /// Weave size of last block.
    weave_base: DataIndex,
    /// All extrinsics in recall block.
    extrinsics: Vec<B::Extrinsic>,
    /// Extrinsics root of recall block.
    extrinsics_root: B::Hash,
    /// Index of the extrinsic in which recall byte is located.
    recall_extrinsic_index: ExtrinsicIndex,
}

impl<B: BlockT<Hash = canyon_primitives::Hash>> RecallInfo<B> {
    /// Converts the recall info to a [`TxProofVerifier`].
    pub fn as_tx_proof_verifier(&self) -> TxProofVerifier<B> {
        let recall_extrinsic = self.extrinsics[self.recall_extrinsic_index as usize].clone();
        TxProofVerifier::new(
            recall_extrinsic,
            self.extrinsics_root,
            self.recall_extrinsic_index,
        )
    }
}

/// Returns all the information about the recall block for the PoA consensus.
fn find_recall_info<Block, Client>(
    recall_byte: DataIndex,
    recall_block_number: NumberFor<Block>,
    client: &Arc<Client>,
) -> Result<RecallInfo<Block>, Error<Block>>
where
    Block: BlockT,
    Client: BlockBackend<Block> + ProvideRuntimeApi<Block> + Send + Sync,
    Client::Api: PermastoreApi<Block, NumberFor<Block>, u32, Block::Hash>,
{
    let recall_block_id = BlockId::number(recall_block_number);

    let (header, extrinsics) = fetch_block(client, recall_block_id)?.deconstruct();

    let weave_base = client
        .runtime_api()
        .weave_size(&BlockId::Hash(*header.parent_hash()))?;

    let mut sized_extrinsics = Vec::with_capacity(extrinsics.len());

    let mut acc = 0u64;
    for (index, _extrinsic) in extrinsics.iter().enumerate() {
        let tx_size = client.runtime_api().data_size(
            &recall_block_id,
            recall_block_number,
            index as ExtrinsicIndex,
        )? as u64;
        if tx_size > 0 {
            sized_extrinsics.push((index as ExtrinsicIndex, weave_base + acc + tx_size));
            acc += tx_size;
        }
    }

    log::trace!(
        target: "poa",
        "The sized extrinsics found in recall block: {:?}",
        sized_extrinsics,
    );

    // No data store transactions in this block.
    if sized_extrinsics.is_empty() {
        return Err(Error::<Block>::RecallExtrinsicNotFound(recall_byte));
    }

    let (recall_extrinsic_index, _recall_block_data_ceil) =
        find_recall_tx(recall_byte, &sized_extrinsics);

    Ok(RecallInfo {
        weave_base,
        extrinsics,
        extrinsics_root: *header.extrinsics_root(),
        recall_extrinsic_index,
    })
}

/// Returns the header and body of block `id`.
fn fetch_block<Block, Client>(
    client: &Arc<Client>,
    id: BlockId<Block>,
) -> Result<Block, Error<Block>>
where
    Block: BlockT,
    Client: BlockBackend<Block>,
{
    Ok(client.block(&id)?.ok_or(Error::BlockNotFound(id))?.block)
}

/// Returns the block number of recall block.
fn find_recall_block<Block: BlockT, RA>(
    at: BlockId<Block>,
    recall_byte: DataIndex,
    runtime_api: &Arc<RA>,
) -> Result<NumberFor<Block>, Error<Block>>
where
    RA: ProvideRuntimeApi<Block> + Send + Sync,
    RA::Api: PermastoreApi<Block, NumberFor<Block>, u32, Block::Hash>,
{
    runtime_api
        .runtime_api()
        .find_recall_block(&at, recall_byte)?
        .ok_or(Error::RecallBlockNotFound(recall_byte))
}

/// A builder for creating [`PoaOutcome`].
pub struct PoaBuilder<Block, Client, TransactionDataBackend> {
    client: Arc<Client>,
    transaction_data_backend: TransactionDataBackend,
    phatom: PhantomData<Block>,
}

impl<Block, Client, TransactionDataBackend> PoaBuilder<Block, Client, TransactionDataBackend>
where
    Block: BlockT<Hash = canyon_primitives::Hash> + 'static,
    Client: BlockBackend<Block>
        + HeaderBackend<Block>
        + ProvideRuntimeApi<Block>
        + Send
        + Sync
        + 'static,
    Client::Api: PermastoreApi<Block, NumberFor<Block>, u32, Block::Hash> + PoaApi<Block>,
    TransactionDataBackend: TransactionDataBackendT<Block>,
{
    /// Creates a new instance of [`PoaBuilder`].
    pub fn new(client: Arc<Client>, transaction_data_backend: TransactionDataBackend) -> Self {
        Self {
            client,
            transaction_data_backend,
            phatom: PhantomData::<Block>,
        }
    }

    /// Returns the number of recall block.
    fn find_recall_block(
        &self,
        at: BlockId<Block>,
        recall_byte: DataIndex,
    ) -> Result<NumberFor<Block>, Error<Block>> {
        self.client
            .runtime_api()
            .find_recall_block(&at, recall_byte)?
            .ok_or(Error::RecallBlockNotFound(recall_byte))
    }

    /// Creates the inherent data [`PoaOutcome`].
    pub fn build(&self, parent: Block::Hash) -> Result<PoaOutcome, Error<Block>> {
        log::debug!(target: "poa", "Start building poa on top of {:?}", parent);
        let parent_id = BlockId::Hash(parent);

        let weave_size = self.client.runtime_api().weave_size(&parent_id)?;

        if weave_size == 0 {
            log::debug!(target: "poa", "Skipping the poa construction as the weave size is 0");
            return Ok(PoaOutcome::Skipped);
        }

        let PoaConfiguration {
            max_depth,
            max_tx_path,
            max_chunk_path,
        } = self.client.runtime_api().poa_config(&parent_id)?;

        for depth in MIN_DEPTH..=max_depth {
            let recall_byte = calculate_challenge_byte(parent.encode(), weave_size, depth);
            log::debug!(
                target: "poa",
                "Attempting to generate poa at depth: {}, recall byte found: {}",
                depth, recall_byte,
            );
            let recall_block_number = self.find_recall_block(parent_id, recall_byte)?;

            log::debug!(
                target: "poa", "Recall block number: {} was found given the recall byte: {}",
                recall_block_number,
                recall_byte,
            );

            let RecallInfo {
                weave_base,
                extrinsics,
                extrinsics_root,
                recall_extrinsic_index,
            } = find_recall_info(recall_byte, recall_block_number, &self.client)?;

            // Continue if the recall tx has been forgotten as the forgot
            // txs can not participate in the consensus.
            //
            // FIXME: handle the data oblivion
            // if todo!("recall_tx has been forgotten via runtime api") {
            // continue;
            // }

            let recall_data = self.transaction_data_backend.transaction_data(
                BlockId::Number(recall_block_number),
                recall_extrinsic_index as u32,
            );

            match recall_data {
                Ok(Some(tx_data)) => {
                    let transaction_data_offset = match recall_byte.checked_sub(weave_base) {
                        Some(offset) => offset,
                        None => panic!(
                            "Underflow happened! recall_byte: {}, recall_block_weave_base: {}",
                            recall_byte, weave_base
                        ),
                    };

                    if let Ok(chunk_proof) =
                        ChunkProofBuilder::new(tx_data, CHUNK_SIZE, transaction_data_offset as u32)
                            .build()
                    {
                        if chunk_proof.size() > max_chunk_path as usize {
                            log::debug!(
                                target: "poa",
                                "Dropping the chunk proof as it's too large ({} > {})",
                                chunk_proof.size(),
                                max_chunk_path,
                            );
                            continue;
                        }

                        if let Ok(tx_proof) = build_extrinsic_proof::<Block>(
                            recall_extrinsic_index,
                            extrinsics_root,
                            extrinsics,
                        ) {
                            let tx_path_size: usize = tx_proof.iter().map(|t| t.len()).sum();
                            if tx_path_size > max_tx_path as usize {
                                log::debug!(
                                    target: "poa",
                                    "Dropping the tx proof as it's too large ({} > {})",
                                    tx_path_size,
                                    max_tx_path,
                                );
                                continue;
                            }
                            let poa = ProofOfAccess::new(depth, tx_proof, chunk_proof);
                            log::trace!(target: "poa", "Generate the poa proof successfully: {:?}", poa);
                            return Ok(PoaOutcome::Justification(poa));
                        }
                    }
                }
                Ok(None) => {
                    log::warn!(
                        target: "poa",
                        "Transaction data not found given block {} and extrinsic index {}, continuing next depth: {}",
                        recall_block_number,
                        recall_extrinsic_index,
                        depth + 1
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

        log::warn!(target: "poa", "Failed to create a poa as the max depth: {} has been reached", max_depth);

        Ok(PoaOutcome::MaxDepthReached(max_depth))
    }
}

/// Returns a [`PoaOutcome`] after the poa construction.
pub fn construct_poa<Block, Client, TransactionDataBackend>(
    client: Arc<Client>,
    parent: Block::Hash,
    transaction_data_backend: TransactionDataBackend,
) -> Result<PoaOutcome, Error<Block>>
where
    Block: BlockT<Hash = canyon_primitives::Hash> + 'static,
    Client: BlockBackend<Block>
        + HeaderBackend<Block>
        + ProvideRuntimeApi<Block>
        + Send
        + Sync
        + 'static,
    Client::Api: PermastoreApi<Block, NumberFor<Block>, u32, Block::Hash> + PoaApi<Block>,
    TransactionDataBackend: TransactionDataBackendT<Block>,
{
    PoaBuilder::new(client, transaction_data_backend).build(parent)
}

/// Extracts PoA digest from a header that should contain one.
///
/// The header should have one and only one [`DigestItem::PreRuntime(POA_ENGINE_ID, pre_runtime)`].
fn fetch_poa<B: BlockT>(header: B::Header, hash: B::Hash) -> Result<ProofOfAccess, Error<B>> {
    use DigestItem::PreRuntime;

    let poa_seal = header
        .digest()
        .logs()
        .iter()
        .filter(|digest_item| matches!(digest_item, PreRuntime(id, _seal) if id == &POA_ENGINE_ID))
        .collect::<Vec<_>>();

    match poa_seal.len() {
        0 => Err(Error::<B>::NoDigest(hash)),
        1 => match poa_seal[0] {
            PreRuntime(_id, seal) => {
                Decode::decode(&mut seal.as_slice()).map_err(Error::<B>::Codec)
            }
            _ => unreachable!("Only items using POA_ENGINE_ID has been filtered; qed"),
        },
        _ => Err(Error::<B>::MultipleDigests(hash)),
    }
}

/// A pure block importer for PoA.
///
/// This importer has to be used with other mature block importer
/// together, e.g., grandpa block import, for it only verifies the
/// validity of PoA sealed digest item in the header and nothing else.
pub struct PurePoaBlockImport<B, I, C, S> {
    inner: I,
    select_chain: S,
    client: Arc<C>,
    phatom: PhantomData<B>,
}

impl<B: Clone, I: Clone, C, S: Clone> Clone for PurePoaBlockImport<B, I, C, S> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            select_chain: self.select_chain.clone(),
            client: self.client.clone(),
            phatom: self.phatom,
        }
    }
}

impl<B, I, C, S> PurePoaBlockImport<B, I, C, S>
where
    B: BlockT,
    I: BlockImport<B, Transaction = sp_api::TransactionFor<C, B>> + Send + Sync,
    I::Error: Into<ConsensusError>,
    C: ProvideRuntimeApi<B> + Send + Sync + HeaderBackend<B> + AuxStore + ProvideCache<B> + BlockOf,
    C::Api: BlockBuilderApi<B>,
{
    /// Creates a new block import suitable to be used in PoA.
    pub fn new(inner: I, client: Arc<C>, select_chain: S) -> Self {
        Self {
            inner,
            client,
            select_chain,
            phatom: PhantomData::<B>,
        }
    }
}

#[async_trait::async_trait]
impl<B, I, C, S> BlockImport<B> for PurePoaBlockImport<B, I, C, S>
where
    B: BlockT<Hash = canyon_primitives::Hash>,
    I: BlockImport<B, Transaction = sp_api::TransactionFor<C, B>> + Send + Sync,
    I::Error: Into<ConsensusError>,
    S: SelectChain<B>,
    C: ProvideRuntimeApi<B>
        + Send
        + Sync
        + BlockBackend<B>
        + HeaderBackend<B>
        + AuxStore
        + ProvideCache<B>
        + BlockOf,
    C::Api: BlockBuilderApi<B> + PermastoreApi<B, NumberFor<B>, u32, B::Hash> + PoaApi<B>,
{
    type Error = ConsensusError;
    type Transaction = sp_api::TransactionFor<C, B>;

    async fn check_block(
        &mut self,
        block: BlockCheckParams<B>,
    ) -> Result<ImportResult, Self::Error> {
        self.inner.check_block(block).await.map_err(Into::into)
    }

    async fn import_block(
        &mut self,
        block: BlockImportParams<B, Self::Transaction>,
        new_cache: HashMap<CacheKeyId, Vec<u8>>,
    ) -> Result<ImportResult, Self::Error> {
        let best_header = self
            .select_chain
            .best_chain()
            .await
            .map_err(|e| format!("Fetch best chain failed via select chain: {:?}", e))?;

        let best_hash = best_header.hash();

        if self
            .client
            .runtime_api()
            .require_proof_of_access(&BlockId::Hash(best_hash))
            .map_err(Error::<B>::ApiError)?
        {
            let header = block.post_header();
            let poa = fetch_poa::<B>(header, best_hash)?;

            let parent_hash = *block.header.parent_hash();
            let poa_config = self
                .client
                .runtime_api()
                .poa_config(&BlockId::Hash(parent_hash))
                .map_err(Error::<B>::ApiError)?;

            poa.check_validity(&poa_config)
                .map_err(Error::<B>::InvalidPoa)?;

            let weave_size = self
                .client
                .runtime_api()
                .weave_size(&BlockId::Hash(parent_hash))
                .map_err(Error::<B>::ApiError)?;

            let ProofOfAccess {
                depth,
                tx_path,
                chunk_proof,
            } = poa;

            let recall_byte = calculate_challenge_byte(parent_hash.encode(), weave_size, depth);
            let recall_block_number =
                find_recall_block(BlockId::Hash(parent_hash), recall_byte, &self.client)?;

            let recall_info = find_recall_info(recall_byte, recall_block_number, &self.client)?;

            recall_info
                .as_tx_proof_verifier()
                .verify(&tx_path)
                .map_err(Error::<B>::VerifyFailed)?;

            let chunk_root = self
                .client
                .runtime_api()
                .chunk_root(
                    &BlockId::Hash(parent_hash),
                    recall_block_number,
                    recall_info.recall_extrinsic_index,
                )
                .map_err(Error::<B>::ApiError)?
                .ok_or(Error::<B>::ChunkRootNotFound(
                    BlockId::Number(recall_block_number),
                    recall_info.recall_extrinsic_index,
                ))?;

            chunk_proof::ChunkProofVerifier::new(chunk_proof)
                .verify(&chunk_root)
                .map_err(Error::<B>::VerifyFailed)?;
        }

        self.inner
            .import_block(block, new_cache)
            .await
            .map_err(Into::into)
    }
}
