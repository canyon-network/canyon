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

use jsonrpc_core as rpc;

pub type Result<T> = std::result::Result<T, Error>;

/// This type describes the count that excceds the max allowed number.
#[derive(Debug)]
pub struct InvalidCount {
    /// Provided value
    pub provided: u32,
    /// Maximum allowed value
    pub max: u32,
}

impl std::fmt::Display for InvalidCount {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "provided: {}, max: {}", self.provided, self.max)
    }
}

impl InvalidCount {
    pub fn new(provided: u32, max: u32) -> Self {
        Self { provided, max }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("transaction data already exists")]
    DataExists,
    #[error("chunk data already exists")]
    ChunkExists,
    #[error("transaction data is too large. {}", _0)]
    DataTooLarge(InvalidCount),
    #[error("chunk is too large")]
    ChunkTooLarge,
    #[error("data path is too large")]
    DataPathTooLarge,
    #[error("data size is too large")]
    DataSizeTooLarge,
    #[error("invalid proof: ")]
    InvalidProof,
    #[error("authoring api: {0}")]
    AuthoringApiError(#[from] sc_rpc_api::author::error::Error),
    /// Call to an unsafe RPC was denied.
    #[error("unsafe api: {0}")]
    UnsafeRpcCalled(#[from] sc_rpc_api::UnsafeRpcError),
}

const BASE_ERROR: i64 = 6000;

impl From<Error> for rpc::Error {
    fn from(e: Error) -> Self {
        match e {
            Error::DataExists => rpc::Error {
                code: rpc::ErrorCode::ServerError(BASE_ERROR),
                message: "transaction data already exists".into(),
                data: None,
            },
            Error::ChunkExists => rpc::Error {
                code: rpc::ErrorCode::ServerError(BASE_ERROR + 1),
                message: "chunk data already exists".into(),
                data: None,
            },
            Error::DataTooLarge( invalid_count) => rpc::Error {
                code: rpc::ErrorCode::ServerError(BASE_ERROR + 2),
                message: format!("transaction data is too large. {}", invalid_count),
                data: Some("the transaction data has to be uploaded or downloaded chunk by chunk for being too large.".into()),
            },
            Error::ChunkTooLarge => rpc::Error {
                code: rpc::ErrorCode::ServerError(BASE_ERROR + 3),
                message: "chunk data is too large".into(),
                data: None,
            },
            Error::DataPathTooLarge => rpc::Error {
                code: rpc::ErrorCode::ServerError(BASE_ERROR + 4),
                message: "data path is too large".into(),
                data: None,
            },
            Error::DataSizeTooLarge => rpc::Error {
                code: rpc::ErrorCode::ServerError(BASE_ERROR + 5),
                message: "data size is too large".into(),
                data: None,
            },
            Error::InvalidProof => rpc::Error {
                code: rpc::ErrorCode::ServerError(BASE_ERROR + 6),
                message: "chunk proof is invalid".into(),
                data: None,
            },
            Error::AuthoringApiError(e) => rpc::Error {
                code: rpc::ErrorCode::ServerError(BASE_ERROR + 7),
                message: e.to_string(),
                data: None,
            },
            Error::UnsafeRpcCalled(e) => e.into(),
        }
    }
}
