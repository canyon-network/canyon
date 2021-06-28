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

pub const POA_ENGINE_ID: [u8; 4] = *b"poa_";

/// 256KiB per chunk.
pub const CHUNK_SIZE: u64 = 256 * 1024 * 1024;

/// Hasher type for permastore.
pub type Hasher = sp_core::Blake2Hasher;

/// Trie layout used for permastore.
pub type TrieLayout = sp_trie::Layout<Hasher>;

/// Error type of chunk proof verification.
pub type VerifyError = sp_trie::VerifyError<sp_core::H256, sp_trie::Error>;
