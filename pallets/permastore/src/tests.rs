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

use frame_support::traits::OnFinalize;

use crate::{
    mock::{new_test_ext, Permastore, Test},
    *,
};

#[test]
fn find_recall_block_should_work() {
    new_test_ext().execute_with(|| {
        BlockDataSize::<Test>::put(5);
        <WeaveSize<Test>>::put(5);

        // [5]
        // [1]
        <Permastore as OnFinalize<u64>>::on_finalize(1);

        BlockDataSize::<Test>::put(7);
        <WeaveSize<Test>>::put(5 + 7);

        // [5, 12]
        // [1, 4]
        <Permastore as OnFinalize<u64>>::on_finalize(4);

        BlockDataSize::<Test>::put(10);
        <WeaveSize<Test>>::put(5 + 7 + 10);

        // [5, 12, 22]
        // [1, 4, 10]
        <Permastore as OnFinalize<u64>>::on_finalize(10);

        assert_eq!(Pallet::<Test>::find_recall_block(3), Some(1));
        assert_eq!(Pallet::<Test>::find_recall_block(12), Some(4));
        assert_eq!(Pallet::<Test>::find_recall_block(13), Some(10));
        assert_eq!(Pallet::<Test>::find_recall_block(15), Some(10));
    });
}
