use cp_consensus_poa::{ChunkProof, ProofOfAccess};

use thiserror::Error;

use canyon_primitives::Depth;

use crate::MAX_DEPTH;

#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid depth: {0}")]
    InvalidDepth(Depth),
}

pub fn verify(poa: ProofOfAccess) -> Result<(), Error> {
    let ProofOfAccess {
        depth,
        tx_path,
        chunk_proof,
    } = poa;

    if depth > MAX_DEPTH {
        return Err(Error::InvalidDepth(depth));
    }

    todo!()
}
