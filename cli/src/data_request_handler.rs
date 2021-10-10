// Copyright 2020 Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! Helper for handling (i.e. answering) state requests from a remote peer via the
//! [`crate::request_responses::RequestResponsesBehaviour`].

use codec::{Decode, Encode};
use futures::{
    channel::{mpsc, oneshot},
    stream::StreamExt,
};
use log::{debug, trace};
use sc_network::{config::ProtocolId, PeerId, ReputationChange};
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::{
    hash::{Hash, Hasher},
    sync::Arc,
    time::Duration,
};

use cc_network::protocol::request_response::{
    ChunkFetchingResponse, ChunkResponse, IncomingRequest, IncomingRequestReceiver,
    OutgoingResponseSender,
};
use cc_network::protocol::{request_response::ChunkFetchingRequest, IsRequest};

use cp_permastore::{PermaStorage, CHUNK_SIZE};

const LOG_TARGET: &str = "data::sync";
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024; // Actual reponse may be bigger.
const MAX_NUMBER_OF_SAME_REQUESTS_PER_PEER: usize = 2;

mod rep {
    use sc_network::ReputationChange as Rep;

    /// Reputation change when a peer sent us the same request multiple times.
    pub const SAME_REQUEST: Rep = Rep::new(i32::MIN, "Same state request multiple times");
}

/// The key of [`BlockRequestHandler::seen_requests`].
#[derive(Eq, PartialEq, Clone)]
struct SeenRequestsKey<B: BlockT> {
    peer: PeerId,
    block: B::Hash,
    start: Vec<u8>,
}

#[allow(clippy::derive_hash_xor_eq)]
impl<B: BlockT> Hash for SeenRequestsKey<B> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.peer.hash(state);
        self.block.hash(state);
        self.start.hash(state);
    }
}

/// The value of [`DataRequestHandler::seen_requests`].
enum SeenRequestsValue {
    /// First time we have seen the request.
    First,
    /// We have fulfilled the request `n` times.
    Fulfilled(usize),
}

/// Handler for incoming data requests from a remote peer.
pub struct DataRequestHandler<S> {
    storage: S,
    request_receiver: IncomingRequestReceiver<ChunkFetchingRequest>,
}

impl<S> DataRequestHandler<S>
where
    S: PermaStorage,
{
    /// Create a new [`DataRequestHandler`].
    pub fn new(
        storage: S,
        request_receiver: IncomingRequestReceiver<ChunkFetchingRequest>,
    ) -> Self {
        Self {
            storage,
            request_receiver,
        }
    }

    /// Run [`DataRequestHandler`].
    pub async fn run(mut self) {
        while let Ok(request) = self.request_receiver.recv(|| vec![]).await {
            let IncomingRequest {
                peer,
                payload,
                pending_response,
            } = request;

            match self.handle_request(payload, pending_response, &peer) {
                Ok(()) => debug!(
                    target: LOG_TARGET,
                    "Handled data chunk request from {}.", peer
                ),
                Err(e) => debug!(
                    target: LOG_TARGET,
                    "Failed to handle data chunk request from {}: {}", peer, e,
                ),
            }
        }
    }

    // FIXME: handle the request properly
    fn handle_request(
        &mut self,
        request: ChunkFetchingRequest,
        pending_response: OutgoingResponseSender<ChunkFetchingRequest>,
        peer: &PeerId,
    ) -> Result<(), HandleRequestError> {
        log::debug!(
            target: LOG_TARGET,
            "---------- Received data chunk request: {:?}",
            request
        );

        let ChunkFetchingRequest { chunk_root, index } = request;

        let maybe_data = self.storage.retrieve(chunk_root.encode().as_slice());

        match maybe_data {
            Some(data) => {
                let chunk_response = ChunkResponse {
                    chunk: data,
                    proof: vec![b"mocked proof".to_vec()], // TODO: add chunk proof
                };
                let chunk_fetching_response = ChunkFetchingResponse::Chunk(chunk_response);

                log::debug!(
                    target: LOG_TARGET,
                    "---------- Sending back response: {:?}",
                    chunk_fetching_response
                );
                pending_response
                    .send_response(chunk_fetching_response)
                    .map_err(|_| HandleRequestError::SendResponse)
            }
            None => pending_response
                .send_response(ChunkFetchingResponse::NoSuchChunk)
                .map_err(|_| HandleRequestError::SendResponse),
        }
    }
}

#[derive(thiserror::Error, Debug)]
enum HandleRequestError {
    #[error("Failed to decode block hash: {0}")]
    InvalidHash(#[from] codec::Error),
    // #[error("Client error: {:?}", _0)]
    // Client(#[from] sp_blockchain::Error),
    #[error("Failed to send response.")]
    SendResponse,
}
