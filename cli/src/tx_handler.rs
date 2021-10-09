use canyon_primitives::Block;
use canyon_runtime::{Call, PermastoreCall, UncheckedExtrinsic};
use cc_network::protocol::request_response::ChunkFetchingRequest;
use codec::{Codec, Decode, Encode};
use futures::prelude::*;
use log::{debug, error};
use sc_network::{IfDisconnected, NetworkService, PeerId, RequestFailure};
use sp_runtime::traits::Block as BlockT;
use std::sync::Arc;

pub struct NewTransactionHandle<E> {
    pub network: Arc<NetworkService<Block, <Block as BlockT>::Hash>>,
    pub receiver: futures::channel::mpsc::UnboundedReceiver<(PeerId, E)>,
}

impl<E: Codec> NewTransactionHandle<E> {
    pub async fn on_new_transaction(&mut self) {
        while let Some((who, new_transaction)) = self.receiver.next().await {
            debug!(target: "sync::data", "Received new_transaction: {:?}", new_transaction.encode());
            let encoded = new_transaction.encode();
            let maybe_uxt: Result<UncheckedExtrinsic, codec::Error> =
                Decode::decode(&mut encoded.as_slice());
            match maybe_uxt {
                Ok(uxt) => match uxt.function {
                    Call::Permastore(permastore_call) => match permastore_call {
                        PermastoreCall::store { .. } => {
                            debug!(target: "sync::data", "Should checkout the local storage and send the data sync request");
                            debug!(target: "sync::data", "Sending the data sync request");
                            match self.send_request(who).await {
                                Ok(res) => {
                                    debug!(target: "sync::data", "----------------- Received response: {:?}", res)
                                }
                                Err(e) => {
                                    error!(target: "sync::data", "------------------ Received error: {:?}", e)
                                }
                            }
                        }
                        call @ _ => {
                            debug!(target: "sync::data", "Ignoring permastore call: {:?}", call)
                        }
                    },
                    Call::Balances(_) => {
                        debug!(target: "sync::data", "Sending the test request-response......");
                        match self.send_request(who).await {
                            Ok(res) => {
                                debug!(target: "sync::data", "----------------- Received response: {:?}", res)
                            }
                            Err(e) => {
                                error!(target: "sync::data", "------------------ Received error: {:?}", e)
                            }
                        }
                        // TODO: request transaction data
                    }
                    call => debug!(target: "sync::data", "Ignoring call: {:?}", call),
                },
                Err(e) => {
                    error!(target: "sync::data", "Failed to decode: {:?}, error: {:?}", encoded, e);
                }
            }
        }
    }

    async fn send_request(&self, target: PeerId) -> Result<Vec<u8>, RequestFailure> {
        let chunk_fetching_protocol = cc_network::protocol::Protocol::ChunkFetching;

        let request = ChunkFetchingRequest {
            chunks_root: sp_core::H256::default(),
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
