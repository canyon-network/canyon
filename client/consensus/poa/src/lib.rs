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

use codec::Encode;
use sp_runtime::{ConsensusEngineId, DigestItem};
use thiserror::Error;

use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder as BlockBuilderApi;
use sp_blockchain::{well_known_cache_keys::Id as CacheKeyId, HeaderBackend, ProvideCache};
use sp_consensus::{
    BlockCheckParams, BlockImport, BlockImportParams, CanAuthorWith, Error as ConsensusError,
    ImportResult, SelectChain,
};
use sp_inherents::{CreateInherentDataProviders, InherentDataProvider};
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
    #[error(transparent)]
    ApiError(#[from] sp_api::ApiError),
    #[error("Block {0} not found")]
    BlockNotFound(BlockId<Block>),
    #[error("Recall block not found given the recall byte {0}")]
    RecallBlockNotFound(DataIndex),
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

impl<B: BlockT> std::convert::From<Error<B>> for ConsensusError {
    fn from(error: Error<B>) -> ConsensusError {
        ConsensusError::ClientImport(error.to_string())
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
        "Finding the recall block at: {:?}, recall_byte: {:?}",
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
    RA::Api: PermastoreApi<Block, NumberFor<Block>, u32, Block::Hash>,
{
    let parent_id = BlockId::Hash(parent);

    let weave_size = runtime_api.runtime_api().weave_size(&parent_id)?;

    if weave_size == 0 {
        log::debug!(target: "poa", "Skipping the poa construction as the weave size is 0");
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
                        "Underflow happened! recall_byte: {}, recall_block_weave_base: {}",
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
                        log::debug!(target: "poa", "Generate the poa proof successfully: {:?}", poa);
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

/// Fetch PoA seal.
fn fetch_seal<B: BlockT>(header: B::Header, hash: B::Hash) -> Result<Vec<u8>, Error<B>> {
    let poa_seal = header
        .digest()
        .logs()
        .iter()
        .filter(|digest_item| {
            log::debug!(target: "poa", "[fetch_seal] digest_item: {:?}", digest_item);
            matches!(digest_item, DigestItem::Seal(id, _seal) if id == &POA_ENGINE_ID)
        })
        .collect::<Vec<_>>();

    match poa_seal.len() {
        0 => Err(Error::<B>::HeaderUnsealed(hash).into()),
        1 => match poa_seal[0] {
            DigestItem::Seal(_id, seal) => Ok(seal.clone()),
            _ => unreachable!("Only Seal with poa engine id has been filtered"),
        },
        _ => Err(Error::<B>::HeaderMultiSealed(hash).into()),
    }
}

/// A block importer for PoA.
pub struct PoaBlockImport<B, I, C, S> {
    inner: I,
    select_chain: S,
    client: Arc<C>,
    phatom: PhantomData<B>,
}

impl<B: Clone, I: Clone, C, S: Clone> Clone for PoaBlockImport<B, I, C, S> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            select_chain: self.select_chain.clone(),
            client: self.client.clone(),
            phatom: self.phatom.clone(),
        }
    }
}

impl<B, I, C, S> PoaBlockImport<B, I, C, S>
where
    B: BlockT,
    I: BlockImport<B, Transaction = sp_api::TransactionFor<C, B>> + Send + Sync,
    I::Error: Into<ConsensusError>,
    C: ProvideRuntimeApi<B> + Send + Sync + HeaderBackend<B> + AuxStore + ProvideCache<B> + BlockOf,
    C::Api: BlockBuilderApi<B>,
{
    /// Create a new block import suitable to be used in PoW
    pub fn new(inner: I, client: Arc<C>, select_chain: S) -> Self {
        Self {
            inner,
            client,
            select_chain,
            phatom: PhantomData::<B>,
        }
    }

    async fn check_inherents(&self, block: B, block_id: BlockId<B>) -> Result<(), Error<B>> {
        /*
        if *block.header().number() < self.check_inherents_after {
            return Ok(());
        }

        if let Err(e) = self.can_author_with.can_author_with(&block_id) {
            log::debug!(
                target: "pow",
                "Skipping `check_inherents` as authoring version is not compatible: {}",
                e,
            );

            return Ok(());
        }

        let inherent_data = inherent_data_providers
            .create_inherent_data()
            .map_err(|e| Error::CreateInherents(e))?;

        let inherent_res = self
            .client
            .runtime_api()
            .check_inherents(&block_id, block, inherent_data)
            .map_err(|e| Error::Client(e.into()))?;

        if !inherent_res.ok() {
            for (identifier, error) in inherent_res.into_errors() {
                match inherent_data_providers
                    .try_handle_error(&identifier, &error)
                    .await
                {
                    Some(res) => res.map_err(Error::CheckInherents)?,
                    None => return Err(Error::CheckInherentsUnknownError(identifier)),
                }
            }
        }
        */

        Ok(())
    }
}

#[async_trait::async_trait]
impl<B, I, C, S> BlockImport<B> for PoaBlockImport<B, I, C, S>
where
    B: BlockT,
    I: BlockImport<B, Transaction = sp_api::TransactionFor<C, B>> + Send + Sync,
    I::Error: Into<ConsensusError>,
    S: SelectChain<B>,
    C: ProvideRuntimeApi<B> + Send + Sync + HeaderBackend<B> + AuxStore + ProvideCache<B> + BlockOf,
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
        mut block: BlockImportParams<B, Self::Transaction>,
        new_cache: HashMap<CacheKeyId, Vec<u8>>,
    ) -> Result<ImportResult, Self::Error> {
        let best_header = self
            .select_chain
            .best_chain()
            .await
            .map_err(|e| format!("Fetch best chain failed via select chain: {:?}", e))?;

        log::debug!(
            target: "poa",
            "[cc_consensus_poa::import_block] ,,  best_hash: {:?}, parent_hash: {:?}",
            best_header.hash(),
            *block.header.parent_hash(),
        );

        let best_hash = best_header.hash();

        let parent_hash = *block.header.parent_hash();

        /*
        if let Some(inner_body) = block.body.take() {
            let check_block = B::new(block.header.clone(), inner_body);

            self.check_inherents(
                check_block.clone(),
                BlockId::Hash(parent_hash),
                self.create_inherent_data_providers
                    .create_inherent_data_providers(parent_hash, ())
                    .await?,
            )
            .await?;

            block.body = Some(check_block.deconstruct().1);
        }
        */

        // Check if the block has data transactions.
        let block_size = self
            .client
            .runtime_api()
            .block_size(&BlockId::Hash(best_hash))
            .map_err(|e| Error::<B>::ApiError(e))?;

        let weave_size = self
            .client
            .runtime_api()
            .weave_size(&BlockId::Hash(best_hash))
            .map_err(|e| Error::<B>::ApiError(e))?;

        log::debug!(target: "poa", "[cc_consensus_poa::import_block] block_size is {} at block {}", block_size, best_hash);

        if block_size > 0 || weave_size > 0 {
            let header = block.post_header();
            let poa_seal = fetch_seal::<B>(header, best_hash)?;
            // verify_seal
            log::debug!(target: "poa", "TODO: verify PoA seal: {:?}", poa_seal);
        }

        self.inner
            .import_block(block, new_cache)
            .await
            .map_err(Into::into)
    }
}
