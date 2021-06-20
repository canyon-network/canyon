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
use crate::WatchDummy;

#[test]
fn it_works_for_optional_value() {
    new_test_ext().execute_with(|| {
        // Check that GenesisBuilder works properly.
        assert_eq!(Poa::dummy(), Some(42));

        // Check that accumulate works when we have Some value in Dummy already.
        assert_ok!(Poa::accumulate_dummy(Origin::signed(1), 27));
        assert_eq!(Poa::dummy(), Some(69));

        // Check that finalizing the block removes Dummy from storage.
        <Poa as OnFinalize<u64>>::on_finalize(1);
        assert_eq!(Poa::dummy(), None);

        // Check that accumulate works when we Dummy has None in it.
        <Poa as OnInitialize<u64>>::on_initialize(2);
        assert_ok!(Poa::accumulate_dummy(Origin::signed(1), 42));
        assert_eq!(Poa::dummy(), Some(42));
    });
}

#[test]
fn it_works_for_default_value() {
    new_test_ext().execute_with(|| {
        assert_eq!(Poa::foo(), 24);
        assert_ok!(Poa::accumulate_foo(Origin::signed(1), 1));
        assert_eq!(Poa::foo(), 25);
    });
}

#[test]
fn signed_ext_watch_dummy_works() {
    new_test_ext().execute_with(|| {
        let call = <pallet_poa::Call<Test>>::set_dummy(10).into();
        let info = DispatchInfo::default();

        assert_eq!(
            WatchDummy::<Test>(PhantomData)
                .validate(&1, &call, &info, 150)
                .unwrap()
                .priority,
            u64::max_value(),
        );
        assert_eq!(
            WatchDummy::<Test>(PhantomData).validate(&1, &call, &info, 250),
            InvalidTransaction::ExhaustsResources.into(),
        );
    })
}

#[test]
fn weights_work() {
    // must have a defined weight.
    let default_call = <pallet_poa::Call<Test>>::accumulate_dummy(10);
    let info = default_call.get_dispatch_info();
    // aka. `let info = <Call<Test> as GetDispatchInfo>::get_dispatch_info(&default_call);`
    assert_eq!(info.weight, 0);

    // must have a custom weight of `100 * arg = 2000`
    let custom_call = <pallet_poa::Call<Test>>::set_dummy(20);
    let info = custom_call.get_dispatch_info();
    assert_eq!(info.weight, 2000);
}
