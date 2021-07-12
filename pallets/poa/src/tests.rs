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

use frame_support::{
    assert_ok,
    pallet_prelude::PhantomData,
    traits::{OnFinalize, OnInitialize},
    weights::{DispatchInfo, GetDispatchInfo},
};
use sp_runtime::{traits::SignedExtension, transaction_validity::InvalidTransaction};

use crate as pallet_poa;

use crate::mock::{new_test_ext, Origin, Poa, Test};

#[test]
fn it_works_for_optional_value() {
    new_test_ext().execute_with(|| {});
}

#[test]
fn signed_ext_watch_dummy_works() {
    new_test_ext().execute_with(|| {
        // let call = <pallet_poa::Call<Test>>::set_dummy(10).into();
        // let info = DispatchInfo::default();

        // assert_eq!(
        // WatchDummy::<Test>(PhantomData)
        // .validate(&1, &call, &info, 150)
        // .unwrap()
        // .priority,
        // u64::max_value(),
        // );
        // assert_eq!(
        // WatchDummy::<Test>(PhantomData).validate(&1, &call, &info, 250),
        // InvalidTransaction::ExhaustsResources.into(),
        // );
    })
}
