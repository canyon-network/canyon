use super::*;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;

benchmarks! {
    // This will measure the execution time of `accumulate_dummy` for b in [1..1000] range.
    store {
        let data_size in 1 .. 1000;
        let caller = account("caller", 0, 0);
        let chunk_root = Default::default();
        let data = Vec::new();
    }: _ (RawOrigin::Signed(caller), data_size.into(), chunk_root, root)

    // This will measure the execution time of `set_dummy` for b in [1..1000] range.
    forget {
        let b in 1 .. 1000;
        let block_number = 100;
        let extrinsic_index = 3;
    }: set_dummy (RawOrigin::Signed(caller), block_number, extrinsic_index)
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
