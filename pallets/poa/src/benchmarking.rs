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

use super::*;
use cp_consensus_poa::{ChunkProof, PoaConfiguration, ProofOfAccess};
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;
use sp_std::vec;

pub(crate) fn mock_a_data_chunk() -> Vec<u8> {
    const ALPHABET: [u8; 32] = [
        b'q', b'w', b'e', b'r', b't', b'y', b'u', b'i', b'o', b'p', b'[', b']', b'a', b's', b'd',
        b'f', b'g', b'h', b'j', b'k', b'l', b';', b'z', b'x', b'c', b'v', b'b', b'n', b'm', b',',
        b'.', b' ',
    ];

    ALPHABET.repeat(8192)
}

benchmarks! {
    // This will measure the execution time of `deposit` for b in [1..1000] range.
    deposit {
        let chunk = mock_a_data_chunk();

        // Generated by test_generate_proof_of_access()
        let chunk_index = 216u32;
        let proof = vec![vec![66, 0, 128, 180, 130, 227, 5, 251, 16, 249, 197, 243, 141, 167, 120, 84, 226, 47, 93, 191, 61, 61, 139, 214, 144, 4, 221, 111, 249, 57, 4, 88, 83, 133, 227]];
        let chunk_proof = ChunkProof::new(chunk, chunk_index, proof);

        let tx_proof = vec![vec![129, 0, 17, 0, 0, 128, 191, 236, 85, 168, 163, 63, 16, 240, 207, 104, 174, 210, 70, 212, 151, 198, 14, 105, 220, 35, 135, 214, 71, 225, 65, 94, 149, 78, 123, 147, 77, 21], vec![64, 0]];

        let poa = ProofOfAccess::new(1, tx_proof, chunk_proof);
        let poa_outcome = PoaOutcome::Justification(poa);
    }: deposit (RawOrigin::None, poa_outcome)
    verify {
        // TODO: verify deposit
    }

    set_config {
        let new = PoaConfiguration {
            max_depth: 1u32,
            max_tx_path: 100u32,
            max_chunk_path: 100u32
        };
    }: set_config (RawOrigin::Root, new.clone())
    verify {
        assert_eq!(new, Pallet::<T>::poa_config());
    }
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
