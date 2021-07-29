use codec::Encode;

use frame_benchmarking::{benchmarks, impl_benchmark_test_suite, whitelisted_caller};
use frame_support::traits::Currency;
use frame_system::RawOrigin;
use sp_runtime::traits::Hash;

use crate::{Call, Config, Pallet};

benchmarks! {
    store {
        let caller = whitelisted_caller();
        let balance = 10_000u32;
        let data = b"transaction data";
        let min = T::Currency::minimum_balance().max(1u32.into());
        T::Currency::make_free_balance_be(&caller, min * balance.into());
        let chunk_root = T::Hashing::hash(&data.encode()[..]);
    }: store (RawOrigin::Signed(caller), data.len() as u32, chunk_root)
    verify {
        // TODO
    }

    forget {
        let caller: T::AccountId = whitelisted_caller();
        let balance = 10_000u32;
        let min = T::Currency::minimum_balance().max(1u32.into());
        T::Currency::make_free_balance_be(&caller, min * balance.into());
        let block_number: T::BlockNumber = 100u32.into();
        frame_system::Pallet::<T>::set_block_number(block_number);
        let data = b"transaction data";
        let chunk_root = T::Hashing::hash(&data.encode()[..]);
        Pallet::<T>::store(RawOrigin::Signed(caller.clone()).into(), data.len() as u32, chunk_root)?;
        let extrinsic_index = 0u32;
    }: forget (RawOrigin::Signed(caller), block_number, extrinsic_index)
    verify {
        // TODO
    }
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
