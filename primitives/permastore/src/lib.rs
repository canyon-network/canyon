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

#![cfg_attr(not(feature = "std"), no_std)]

use sp_core::offchain::OffchainStorage;

pub const POA_ENGINE_ID: [u8; 4] = *b"poa_";

/// 256KiB per chunk.
pub const DEFAULT_CHUNK_SIZE: u64 = 256 * 1024 * 1024;

/// Hasher type for permastore.
pub type Hasher = sp_core::Blake2Hasher;

///
pub type TrieLayout = sp_trie::Layout<Hasher>;

/// Persistent transaction data storage.
pub trait PermaStorage: Send + Sync + Clone {
    /// Persist a value in storage under given key.
    fn submit(&mut self, key: &[u8], value: &[u8]);

    /// Retrieve a value from storage under given key.
    fn retrieve(&self, key: &[u8]) -> Option<Vec<u8>>;

    /// Checks if the storage exists under given key.
    fn exists(&self, key: &[u8]) -> bool {
        self.retrieve(key).is_some()
    }
}

// PermaStorage backed by offchain storage.
impl<T: OffchainStorage> PermaStorage for T {
    fn submit(&mut self, key: &[u8], value: &[u8]) {
        self.set(sp_offchain::STORAGE_PREFIX, key, value)
    }

    fn retrieve(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.get(sp_offchain::STORAGE_PREFIX, key)
    }
}
