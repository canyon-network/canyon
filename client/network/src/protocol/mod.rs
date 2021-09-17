//! TODO

use std::borrow::Cow;
use std::time::Duration;

use futures::channel::mpsc;
use strum::EnumIter;

use sc_network::config::RequestResponseConfig;

pub mod request_response;

/// A protocol per subsystem seems to make the most sense, this way we don't need any dispatching
/// within protocols.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, EnumIter)]
pub enum Protocol {
    /// Protocol for chunk fetching, used by availability distribution and availability recovery.
    ChunkFetching,
}

/// Minimum bandwidth we expect for validators - 500Mbit/s is the recommendation, so approximately

const MIN_BANDWIDTH_BYTES: u64 = 50 * 1024 * 1024;

/// Default request timeout in seconds.
///
/// When decreasing this value, take into account that the very first request might need to open a
/// connection, which can be slow. If this causes problems, we should ensure connectivity via peer
/// sets.
#[allow(dead_code)]
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

/// Request timeout where we can assume the connection is already open (e.g. we have peers in a
/// peer set as well).
const DEFAULT_REQUEST_TIMEOUT_CONNECTED: Duration = Duration::from_secs(1);

/// Timeout for requesting availability chunks.
pub const CHUNK_REQUEST_TIMEOUT: Duration = DEFAULT_REQUEST_TIMEOUT_CONNECTED;

/// This timeout is based on what seems sensible from a time budget perspective, considering 6
/// second block time. This is going to be tough, if we have multiple forks and large PoVs, but we
/// only have so much time.
const POV_REQUEST_TIMEOUT_CONNECTED: Duration = Duration::from_millis(1000);

/// We want timeout statement requests fast, so we don't waste time on slow nodes. Responders will
/// try their best to either serve within that timeout or return an error immediately. (We need to
/// fit statement distribution within a block of 6 seconds.)
const STATEMENTS_TIMEOUT: Duration = Duration::from_secs(1);

/// We don't want a slow peer to slow down all the others, at the same time we want to get out the
/// data quickly in full to at least some peers (as this will reduce load on us as they then can
/// start serving the data). So this value is a tradeoff. 3 seems to be sensible. So we would need
/// to have 3 slow nodes connected, to delay transfer for others by `STATEMENTS_TIMEOUT`.
pub const MAX_PARALLEL_STATEMENT_REQUESTS: u32 = 3;

impl Protocol {
    /// Get a configuration for a given Request response protocol.
    ///
    /// Returns a receiver for messages received on this protocol and the requested
    /// `ProtocolConfig`.
    pub fn get_config(
        self,
    ) -> (
        mpsc::Receiver<sc_network::config::IncomingRequest>,
        RequestResponseConfig,
    ) {
        let p_name = self.into_protocol_name();
        let (tx, rx) = mpsc::channel(self.get_channel_size());
        let cfg = match self {
            Protocol::ChunkFetching => RequestResponseConfig {
                name: p_name,
                max_request_size: 1_000,
                max_response_size: 10 * 1024 * 1024,
                // We are connected to all validators:
                request_timeout: CHUNK_REQUEST_TIMEOUT,
                inbound_queue: Some(tx),
            },
        };
        (rx, cfg)
    }

    // Channel sizes for the supported protocols.
    fn get_channel_size(self) -> usize {
        match self {
            // Hundreds of validators will start requesting their chunks once they see a candidate
            // awaiting availability on chain. Given that they will see that block at different
            // times (due to network delays), 100 seems big enough to accomodate for "bursts",
            // assuming we can service requests relatively quickly, which would need to be measured
            // as well.
            Self::ChunkFetching => 100,
        }
    }

    /// Get the protocol name of this protocol, as understood by substrate networking.
    pub fn into_protocol_name(self) -> Cow<'static, str> {
        self.get_protocol_name_static().into()
    }

    /// Get the protocol name associated with each peer set as static str.
    pub const fn get_protocol_name_static(self) -> &'static str {
        match self {
            Self::ChunkFetching => "/canyon/req_data_chunk/1",
        }
    }
}

/// Common properties of any `Request`.
pub trait IsRequest {
    /// Each request has a corresponding `Response`.
    type Response;

    /// What protocol this `Request` implements.
    const PROTOCOL: Protocol;
}
