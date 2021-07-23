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

use crate::mock::{new_test_ext, Poa, Test};
use crate::{DepthInfo, HistoryDepth, TestAuthor};

#[test]
fn note_depth_should_work() {
    new_test_ext().execute_with(|| {
        TestAuthor::<Test>::put(6);
        Poa::note_depth(10);
        assert_eq!(
            HistoryDepth::<Test>::get(&6).unwrap(),
            DepthInfo {
                total_depth: 10,
                blocks: 1
            }
        );

        TestAuthor::<Test>::put(8);
        Poa::note_depth(1);
        assert_eq!(
            HistoryDepth::<Test>::get(&8).unwrap(),
            DepthInfo {
                total_depth: 1,
                blocks: 1
            }
        );

        TestAuthor::<Test>::put(6);
        Poa::note_depth(1);
        assert_eq!(
            HistoryDepth::<Test>::get(&6).unwrap(),
            DepthInfo {
                total_depth: 11,
                blocks: 2
            }
        );
    });
}
