use canyon_primitives::Block;
use canyon_runtime::{Call, PermastoreCall, UncheckedExtrinsic};
use cc_network::protocol::request_response::{
    ChunkFetchingRequest, ChunkFetchingResponse, ChunkResponse,
};
use codec::{Codec, Decode, Encode};
use futures::prelude::*;
use log::{debug, error, warn};
use sc_network::{IfDisconnected, NetworkService, PeerId, RequestFailure};
use sp_runtime::traits::Block as BlockT;
use std::sync::Arc;

pub struct NewTransactionHandle<E, S> {
    pub network: Arc<NetworkService<Block, <Block as BlockT>::Hash>>,
    pub receiver: futures::channel::mpsc::UnboundedReceiver<(PeerId, E)>,
    /// Permanent storage instance.
    pub storage: S,
}

impl<E: Codec, S: cp_permastore::PermaStorage> NewTransactionHandle<E, S> {
    pub async fn on_new_transaction(&mut self) {
        while let Some((who, new_transaction)) = self.receiver.next().await {
            debug!(target: "sync::data", "Received new_transaction: {:?}", new_transaction.encode());
            let encoded = new_transaction.encode();
            let maybe_uxt: Result<UncheckedExtrinsic, codec::Error> =
                Decode::decode(&mut encoded.as_slice());
            match maybe_uxt {
                Ok(uxt) => match uxt.function {
                    Call::Permastore(permastore_call) => match permastore_call {
                        PermastoreCall::store {
                            data_size,
                            chunk_root,
                        } => {
                            debug!(target: "sync::data", "Sending the data chunk fetching request...");
                            match self
                                .send_chunk_fetching_request(data_size, chunk_root, who)
                                .await
                            {
                                Ok(res) => {
                                    let chunk_fetching_response =
                                        match ChunkFetchingResponse::decode(&mut res.as_slice()) {
                                            Ok(res) => res,
                                            Err(e) => {
                                                error!(target: "sync::data", "Failed to decode ChunkFetchingResponse: {:?}", e);
                                                continue;
                                            }
                                        };

                                    debug!(
                                        target: "sync::data",
                                        "Received raw response: {:?}, chunk_fetching_response: {:?}",
                                        res, chunk_fetching_response,
                                    );

                                    match chunk_fetching_response {
                                        ChunkFetchingResponse::Chunk(chunk_response) => {
                                            let ChunkResponse { chunk, proof } = chunk_response;
                                            debug!(
                                                target: "sync::data",
                                                "===== Received data chunk: {}, proof: {}",
                                                String::from_utf8_lossy(&chunk), String::from_utf8_lossy(&proof[0]),
                                            );
                                            // TODO: store the data chunk locally
                                            self.storage
                                                .submit(chunk_root.encode().as_slice(), &chunk);
                                        }
                                        ChunkFetchingResponse::NoSuchChunk => {
                                            warn!(target: "sync::data", "No such chunk from peer: {:?}", who);
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!(target: "sync::data", "Received error: {:?}", e)
                                }
                            }
                        }
                        call => {
                            debug!(target: "sync::data", "Ignoring permastore call: {:?}", call)
                        }
                    },
                    call => debug!(target: "sync::data", "Ignoring call: {:?}", call),
                },
                Err(e) => {
                    error!(target: "sync::data", "Failed to decode: {:?}, error: {:?}", encoded, e);
                }
            }
        }
    }

    async fn send_chunk_fetching_request(
        &self,
        data_size: u32,
        chunk_root: <Block as BlockT>::Hash,
        target: PeerId,
    ) -> Result<Vec<u8>, RequestFailure> {
        let chunk_fetching_protocol = cc_network::protocol::Protocol::ChunkFetching;

        let request = ChunkFetchingRequest {
            chunk_root,
            index: 0,
        };

        self.network
            .request(
                target,
                chunk_fetching_protocol.get_protocol_name_static(),
                request.encode(),
                IfDisconnected::ImmediateError,
            )
            .await
    }
}
