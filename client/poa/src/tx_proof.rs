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

use codec::{Decode, Encode};

use sp_core::H256;
use sp_runtime::traits::Hash as HashT;
use sp_trie::TrieMut;

use cp_permastore::{Hasher, TrieLayout, VerifyError, CHUNK_SIZE};

pub fn build_transaction_proof<Hash: HashT>(
    extrinsic_index: usize,
    extrinsics_root: Hash::Output,
) -> Result<Vec<Vec<u8>>, ()> {
    todo!()
}
