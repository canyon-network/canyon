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

use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

use codec::{Decode, Encode};
use sp_runtime::{ConsensusEngineId, DigestItem};
use thiserror::Error;

use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder as BlockBuilderApi;
use sp_blockchain::{well_known_cache_keys::Id as CacheKeyId, HeaderBackend, ProvideCache};
use sp_consensus::{
    BlockCheckParams, BlockImport, BlockImportParams, Error as ConsensusError, ImportResult,
    SelectChain,
};
use sp_runtime::{
    generic::BlockId,
    traits::{Block as BlockT, Header as HeaderT, NumberFor},
};

use sc_client_api::{backend::AuxStore, BlockBackend, BlockOf};

use canyon_primitives::{DataIndex, Depth, ExtrinsicIndex};
use cc_datastore::TransactionDataBackend as TransactionDataBackendT;
use cp_consensus_poa::{PoaOutcome, POA_ENGINE_ID};
use cp_permastore::{PermastoreApi, CHUNK_SIZE};

mod chunk_proof;
mod inherent;
mod tx_proof;

pub use self::chunk_proof::{verify_chunk_proof, ChunkProofBuilder, ChunkProofVerifier};
pub use self::inherent::PoaInherentDataProvider;
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

/// Error type for poa consensus.
#[derive(Error, Debug)]
pub enum Error<Block: BlockT> {
    #[error("Header uses the wrong engine {0:?}")]
    WrongEngine(ConsensusEngineId),
    #[error("Header {0:?} has no PoA seal")]
    HeaderUnsealed(Block::Hash),
    #[error("Header {0:?} has multiple PoA seals")]
    HeaderMultiSealed(Block::Hash),
    #[error("Client error: {0}")]
    Client(sp_blockchain::Error),
    #[error("Codec error")]
    Codec(#[from] codec::Error),
    #[error("Blockchain error")]
    BlockchainError(#[from] sp_blockchain::Error),
    #[error("VerifyError error")]
    VerifyFailed(#[from] cp_permastore::VerifyError),
    #[error(transparent)]
    ApiError(#[from] sp_api::ApiError),
    #[error("Block {0} not found")]
    BlockNotFound(BlockId<Block>),
    #[error("Recall block not found given the recall byte {0}")]
    RecallBlockNotFound(DataIndex),
    #[error("Recall extrinsic index not found given the recall byte {0}")]
    RecallExtrinsicNotFound(DataIndex),
    #[error("Header {0} not found")]
    HeaderNotFound(BlockId<Block>),
    #[error("Creating inherents failed: {0}")]
    CreateInherents(sp_inherents::Error),
    #[error("Checking inherents failed: {0}")]
    CheckInherents(sp_inherents::Error),
    #[error("Checking inherents unknown error for identifier: {0:?}")]
    CheckInherentsUnknownError(sp_inherents::InherentIdentifier),
    #[error("the chunk in recall tx not found")]
    InvalidChunk,
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

/// Returns a tuple of (extrinsic_index, absolute_data_index) of extrinsic
/// in which `recall_byte` is located.
fn find_recall_tx(
    recall_byte: DataIndex,
    sized_extrinsics: &[(ExtrinsicIndex, DataIndex)],
) -> (ExtrinsicIndex, DataIndex) {
    log::trace!(
        target: "poa",
        "Try finding recall tx, recall_byte: {}, sized_extrinsics: {:?}",
        recall_byte, sized_extrinsics
    );
    binary_search(recall_byte, sized_extrinsics)
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
    pub fn into_tx_proof_verifier(self) -> tx_proof::TxProofVerifier<B> {
        let recall_extrinsic = self.extrinsics[self.recall_extrinsic_index as usize].clone();
        tx_proof::TxProofVerifier::new(
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

    let (header, extrinsics) = fetch_block(client, recall_block_id)?;

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
) -> Result<(Block::Header, Vec<Block::Extrinsic>), Error<Block>>
where
    Block: BlockT,
    Client: BlockBackend<Block>,
{
    Ok(client
        .block(&id)?
        .ok_or(Error::BlockNotFound(id))?
        .block
        .deconstruct())
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
    Client::Api: PermastoreApi<Block, NumberFor<Block>, u32, Block::Hash>,
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

    /// Returns the block number of recall block.
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

    pub fn build(&self, parent: Block::Hash) -> Result<PoaOutcome, Error<Block>> {
        log::debug!(target: "poa", "Start building poa on top of {:?}", parent);
        let parent_id = BlockId::Hash(parent);

        let weave_size = self.client.runtime_api().weave_size(&parent_id)?;

        if weave_size == 0 {
            log::debug!(target: "poa", "Skipping the poa construction as the weave size is 0");
            return Ok(PoaOutcome::Skipped);
        }

        for depth in MIN_DEPTH..=MAX_DEPTH {
            log::debug!(target: "poa", "Attempting to generate poa at depth: {}", depth);
            let recall_byte = calculate_challenge_byte(parent.encode(), weave_size, depth);
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
                            extrinsics_root,
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
                            let poa = ProofOfAccess::new(depth, tx_proof, chunk_proof);
                            log::trace!(target: "poa", "Generate the poa proof successfully: {:?}", poa);
                            return Ok(PoaOutcome::Justification(poa));
                        }
                    }
                }
                Ok(None) => {
                    log::warn!(
                        target: "poa",
                        "Transaction data not found given block {} and extrinsic index {}",
                        recall_block_number,
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
    Client::Api: PermastoreApi<Block, NumberFor<Block>, u32, Block::Hash>,
    TransactionDataBackend: TransactionDataBackendT<Block>,
{
    PoaBuilder::new(client, transaction_data_backend).build(parent)
}

/// Extracts PoA seal from a header that is expected to contain a poa proof.
///
/// The header should have one and only one [`DigestItem::Seal(POA_ENGINE_ID, seal)`].
fn fetch_poa<B: BlockT>(header: B::Header, hash: B::Hash) -> Result<ProofOfAccess, Error<B>> {
    let poa_seal = header
        .digest()
        .logs()
        .iter()
        .filter(|digest_item| matches!(digest_item, DigestItem::Seal(id, _seal) if id == &POA_ENGINE_ID))
        .collect::<Vec<_>>();

    match poa_seal.len() {
        0 => Err(Error::<B>::HeaderUnsealed(hash)),
        1 => match poa_seal[0] {
            DigestItem::Seal(_id, seal) => {
                Decode::decode(&mut seal.as_slice()).map_err(Error::<B>::Codec)
            }
            _ => unreachable!("Only items sealed using POA_ENGINE_ID has been filtered; qed"),
        },
        _ => Err(Error::<B>::HeaderMultiSealed(hash)),
    }
}

/// A block importer for PoA.
///
/// This importer has to be used with other mature block importer
/// togather, e.g., grandpa block import, for it only verifies the
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
    C::Api: BlockBuilderApi<B> + PermastoreApi<B, NumberFor<B>, u32, B::Hash>,
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
            let ProofOfAccess {
                depth,
                tx_path,
                chunk_proof,
            } = fetch_poa::<B>(header, best_hash)?;

            let parent_hash = *block.header.parent_hash();

            let weave_size = self
                .client
                .runtime_api()
                .weave_size(&BlockId::Hash(parent_hash))
                .map_err(Error::<B>::ApiError)?;

            let recall_byte = calculate_challenge_byte(parent_hash.encode(), weave_size, depth);
            let recall_block_number =
                find_recall_block(BlockId::Hash(parent_hash), recall_byte, &self.client)?;

            find_recall_info(recall_byte, recall_block_number, &self.client)?
                .into_tx_proof_verifier()
                .verify(&tx_path)
                .map_err(Error::<B>::VerifyFailed)?;

            chunk_proof::ChunkProofVerifier::new(chunk_proof)
                .verify(CHUNK_SIZE as usize)
                .map_err(Error::<B>::VerifyFailed)?;
        }

        self.inner
            .import_block(block, new_cache)
            .await
            .map_err(Into::into)
    }
}
