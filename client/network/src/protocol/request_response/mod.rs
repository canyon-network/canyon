// Copyright 2021 Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

//! Requests and responses as sent over the wire for the individual protocols.

use codec::{Decode, Encode};

use sp_core::H256;

use super::{IsRequest, Protocol};

pub mod incoming;
pub mod outgoing;

pub use incoming::{IncomingRequest, IncomingRequestReceiver, OutgoingResponseSender};
pub use outgoing::{OutgoingRequest, OutgoingResult, Recipient, Requests, ResponseSender};

///
pub type ChunkIndex = u32;
///
pub type Proof = Vec<Vec<u8>>;

/// Request an availability chunk.
#[derive(Debug, Copy, Clone, Encode, Decode)]
pub struct ChunkFetchingRequest {
    /// Root hash of transation chunks we want a chunk for.
    pub chunks_root: H256,
    /// The index of the chunk to fetch.
    pub index: ChunkIndex,
}

///
#[derive(Debug, Clone, Encode, Decode)]
pub enum ChunkFetchingResponse {
    /// The requested chunk data.
    #[codec(index = 0)]
    Chunk(ChunkResponse),
    /// Node was not in possession of the requested chunk.
    #[codec(index = 1)]
    NoSuchChunk,
}

impl From<Option<ChunkResponse>> for ChunkFetchingResponse {
    fn from(x: Option<ChunkResponse>) -> Self {
        match x {
            Some(c) => Self::Chunk(c),
            None => Self::NoSuchChunk,
        }
    }
}

///
#[derive(Debug, Clone, Encode, Decode)]
pub struct ChunkResponse {
    /// The data chunk belonging to the transaction data.
    pub chunk: Vec<u8>,
    /// Proof for this chunk's branch in the Merkle tree.
    pub proof: Proof,
}

impl IsRequest for ChunkFetchingRequest {
    type Response = ChunkFetchingResponse;
    const PROTOCOL: Protocol = Protocol::ChunkFetching;
}
