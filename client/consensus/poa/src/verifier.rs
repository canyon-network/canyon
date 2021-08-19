use sc_consensus::Verifier;

use super::*;

/// A verifier for PoA blocks.
pub struct PoaVerifier<B, C> {
    client: Arc<C>,
    phatom: PhantomData<B>,
}

impl<B, C> PoaVerifier<B, C>
where
    B: BlockT<Hash = canyon_primitives::Hash>,
    C: ProvideRuntimeApi<B> + BlockBackend<B> + Send + Sync,
    C::Api: BlockBuilderApi<B> + PermastoreApi<B, NumberFor<B>, u32, B::Hash> + PoaApi<B>,
{
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            phatom: PhantomData::<B>,
        }
    }

    /// Note:
    /// It's post_header in block_import
    pub fn check_header(
        &self,
        header: B::Header,
        best_hash: B::Hash,
        parent_hash: B::Hash,
    ) -> Result<(), ConsensusError> {
        if self
            .client
            .runtime_api()
            .require_proof_of_access(&BlockId::Hash(best_hash))
            .map_err(Error::<B>::ApiError)?
        {
            let poa = fetch_poa::<B>(header, best_hash)?;

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

        Ok(())
    }
}

#[async_trait::async_trait]
impl<B: BlockT, C> Verifier<B> for PoaVerifier<B, C>
where
    C: ProvideRuntimeApi<B> + Send + Sync,
    C::Api: PermastoreApi<B, NumberFor<B>, u32, B::Hash> + PoaApi<B>,
{
    async fn verify(
        &mut self,
        block: BlockImportParams<B, ()>,
    ) -> Result<(BlockImportParams<B, ()>, Option<Vec<(CacheKeyId, Vec<u8>)>>), String> {
        // let post_header = block.header.hash();
        // self.checked_header(block.post_header())
        // let (checked_header, seal) = self.check_header(block.header)?;

        Ok((block, None))
    }
}
