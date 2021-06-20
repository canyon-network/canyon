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
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;

benchmarks! {
    // This will measure the execution time of `accumulate_dummy` for b in [1..1000] range.
    accumulate_dummy {
        let b in 1 .. 1000;
        let caller = account("caller", 0, 0);
    }: _ (RawOrigin::Signed(caller), b.into())

    // This will measure the execution time of `set_dummy` for b in [1..1000] range.
    set_dummy {
        let b in 1 .. 1000;
    }: set_dummy (RawOrigin::Root, b.into())

    // This will measure the execution time of `set_dummy` for b in [1..10] range.
    another_set_dummy {
        let b in 1 .. 10;
    }: set_dummy (RawOrigin::Root, b.into())

    // This will measure the execution time of sorting a vector.
    sort_vector {
        let x in 0 .. 10000;
        let mut m = Vec::<u32>::new();
        for i in (0..x).rev() {
            m.push(i);
        }
    }: {
        m.sort();
    }
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
