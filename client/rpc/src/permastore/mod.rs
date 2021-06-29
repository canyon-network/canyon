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

use std::ops::Deref;
use std::sync::Arc;

use parking_lot::RwLock;

use sp_core::Bytes;

use cc_rpc_api::permastore::{
    error::{Error, Result},
    PermastoreApi,
};
use cp_permastore::PermaStorage;

#[derive(Debug)]
pub struct Permastore<T: PermaStorage> {
    storage: Arc<RwLock<T>>,
}

impl<T: PermaStorage> Permastore<T> {
    pub fn new(storage: T) -> Self {
        Self {
            storage: Arc::new(RwLock::new(storage)),
        }
    }
}

/// Maximum byte size of uploading transaction data directly. 10MiB
const MAX_UPLOAD_DATA_SIZE: u32 = 10 * 1024 * 1024;

/// Maximum byte size of downloading transaction data directly. 12MiB
const MAX_DOWNLOAD_DATA_SIZE: u32 = 12 * 1024 * 1024;

impl<T: PermaStorage + 'static> PermastoreApi for Permastore<T> {
    /// Submit the transaction data under given key.
    fn submit(&self, key: Bytes, value: Bytes) -> Result<()> {
        let data_size = value.deref().len() as u32;
        if data_size > MAX_UPLOAD_DATA_SIZE {
            return Err(Error::DataTooLarge {
                provided: data_size,
                max: MAX_UPLOAD_DATA_SIZE,
            });
        }
        self.storage.write().submit(&*key, &*value);
        Ok(())
    }

    /// Fetch storage under given key.
    fn retrieve(&self, key: Bytes) -> Result<Option<Bytes>> {
        if let Some(value) = self.storage.read().retrieve(&*key) {
            let data_size = value.len() as u32;
            if data_size > MAX_DOWNLOAD_DATA_SIZE {
                return Err(Error::DataTooLarge {
                    provided: data_size,
                    max: MAX_UPLOAD_DATA_SIZE,
                });
            }
            Ok(Some(value.into()))
        } else {
            Ok(None)
        }
    }
}
