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

//! Market for storing data permanently.
//!
//! Provides the interfaces for storing data onto the network.

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};

use sp_runtime::{
    traits::{AccountIdConversion, DispatchInfoOf, SaturatedConversion, SignedExtension},
    transaction_validity::{InvalidTransaction, TransactionValidity, TransactionValidityError},
};
use sp_std::{marker::PhantomData, prelude::*};

use frame_support::{
    ensure,
    traits::{Currency, ExistenceRequirement, Get, IsSubType},
    weights::Weight,
};
use frame_system::ensure_signed;

// #[cfg(any(feature = "runtime-benchmarks", test))]
// mod benchmarking;
#[cfg(all(feature = "std", test))]
mod mock;
#[cfg(all(feature = "std", test))]
mod tests;

/// The balance type of this module.
pub type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

type ExtrinsicIndex = u32;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, Get},
        PalletId,
    };
    use frame_system::pallet_prelude::{BlockNumberFor, OriginFor};

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// The native currency.
        type Currency: Currency<Self::AccountId>;

        /// The treasury pallet id.
        type TreasuryPalletId: Get<PalletId>;

        /// Maximum of a transaction data in bytes.
        type MaxDataSize: Get<u32>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
            // Clear the block size of last block.
            <BlockDataSize<T>>::kill();
            1
        }

        fn on_finalize(n: BlockNumberFor<T>) {
            let current_block_data_size = <BlockDataSize<T>>::get();
            if current_block_data_size > 0 {
                let latest_weave_size = <WeaveSize<T>>::get();
                <GlobalWeaveSizeIndex<T>>::append(latest_weave_size);
                <GlobalBlockNumberIndex<T>>::append(n);
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Stores the data permanently.
        ///
        /// The minimum data size is 1 bytes, the maximum is `MAX_DATA_SIZE`.
        /// The digest of data will be recorded on chain, the actual data has
        /// to be stored off-chain before executing this extrinsic.
        #[pallet::weight(0)]
        pub fn store(origin: OriginFor<T>, data_size: u32, chunk_root: T::Hash) -> DispatchResult {
            let sender = ensure_signed(origin)?;

            ensure!(
                data_size > 0 && data_size < T::MaxDataSize::get(),
                Error::<T>::InvalidDataSize
            );
            ensure!(Self::stored_locally(&chunk_root), Error::<T>::NotStored);

            // TODO: ensure the validity of stored data in the local DB?

            let storage_fee = Self::charge_storage_fee(&sender, data_size)?;

            let block_number = frame_system::Pallet::<T>::block_number();
            let extrinsic_index = frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default();

            Orders::<T>::insert(&sender, (block_number, extrinsic_index), storage_fee);

            // FIXME: store these info in db directly.
            ChunkRootIndex::<T>::insert((block_number, extrinsic_index), chunk_root);
            TransactionDataSize::<T>::insert((block_number, extrinsic_index), data_size);

            <BlockDataSize<T>>::mutate(|s| *s += data_size as u64);
            <WeaveSize<T>>::mutate(|s| *s += data_size as u64);

            Self::deposit_event(Event::Stored(sender, chunk_root));

            Ok(())
        }

        /// _Delete_ the data from the network by removing the incentive
        /// to keep storing them.
        ///
        /// By the mean of forgetting a data, this piece of data will be
        /// prevented from being selected as the random data source in the
        /// PoA consensus.
        #[pallet::weight(0)]
        pub fn forget(
            origin: OriginFor<T>,
            block_number: T::BlockNumber,
            extrinsic_index: ExtrinsicIndex,
        ) -> DispatchResult {
            let sender = ensure_signed(origin)?;

            // Remove the order.
            let _fee = Orders::<T>::take(&sender, (block_number, extrinsic_index))
                .ok_or(Error::<T>::OrderDoesNotExist)?;

            // refund the remaining fee.
            Self::refund_storage_fee(&sender, block_number);

            Self::deposit_event(Event::Forgot(block_number, extrinsic_index));

            Ok(())
        }
    }

    /// Event for the Permastore pallet.
    #[pallet::event]
    #[pallet::metadata(
        T::AccountId = "AccountId",
        T::BlockNumber = "BlockNumber",
        T::Hash = "Hash",
        ExtrinsicIndex = "u32"
    )]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// New storage order. [who, chunk_root]
        Stored(T::AccountId, T::Hash),
        /// The data has been forgotten. [block_number, extrinsic_index]
        Forgot(T::BlockNumber, ExtrinsicIndex),
    }

    /// Error for the Permastore pallet.
    #[pallet::error]
    pub enum Error<T> {
        /// The valid range of data size is (0, MAX_DATA_SIZE).
        InvalidDataSize,
        /// The transaction data has not been stored locally.
        NotStored,
        /// The storage order does not exist.
        OrderDoesNotExist,
    }

    /// Map of all the storage orders.
    #[pallet::storage]
    #[pallet::getter(fn orders)]
    pub(super) type Orders<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Twox64Concat,
        (T::BlockNumber, ExtrinsicIndex),
        BalanceOf<T>,
    >;

    /// Total byte size of data stored onto the network.
    #[pallet::storage]
    pub(super) type WeaveSize<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// Total byte size of data stored in current building block.
    #[pallet::storage]
    #[pallet::getter(fn block_data_size)]
    pub(super) type BlockDataSize<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// (block_number, extrinsic_index) => Option<chunk_root>
    #[pallet::storage]
    #[pallet::getter(fn chunk_root_index)]
    pub(super) type ChunkRootIndex<T: Config> =
        StorageMap<_, Twox64Concat, (T::BlockNumber, ExtrinsicIndex), T::Hash>;

    /// (block_number, extrinsic_index) => transaction_data_size
    #[pallet::storage]
    #[pallet::getter(fn transaction_data_size)]
    pub(super) type TransactionDataSize<T: Config> =
        StorageMap<_, Twox64Concat, (T::BlockNumber, ExtrinsicIndex), u32, ValueQuery>;

    /// FIXME: find a proper way to store these info.
    ///
    /// Temp solution for locating the recall block. An ever increasing array of global weave size.
    #[pallet::storage]
    #[pallet::getter(fn global_block_size_index)]
    pub(super) type GlobalWeaveSizeIndex<T: Config> = StorageValue<_, Vec<u64>, ValueQuery>;

    /// Temp solution for locating the recall block.
    #[pallet::storage]
    #[pallet::getter(fn global_block_number_index)]
    pub(super) type GlobalBlockNumberIndex<T: Config> =
        StorageValue<_, Vec<T::BlockNumber>, ValueQuery>;
}

impl<T: Config> Pallet<T> {
    // TODO: ensure the transaction data has been indeed stored in the local DB.
    fn stored_locally(_chunk_root: &T::Hash) -> bool {
        true
    }

    // TODO: calculate the perpetual storage cost based on the data size.
    fn calculate_storage_fee(data_size: u32) -> BalanceOf<T> {
        data_size.saturated_into()
    }

    /// Charges the perpetual storage fee.
    ///
    /// TODO: Currently all the fee is simply transfered to the treasury,
    /// we might want a new destination for that.
    fn charge_storage_fee(
        who: &T::AccountId,
        data_size: u32,
    ) -> Result<BalanceOf<T>, sp_runtime::DispatchError> {
        let fee = Self::calculate_storage_fee(data_size);
        let treasury_account: T::AccountId = T::TreasuryPalletId::get().into_account();
        T::Currency::transfer(who, &treasury_account, fee, ExistenceRequirement::KeepAlive)?;
        Ok(fee)
    }

    fn refund_storage_fee(_who: &T::AccountId, _created_at: T::BlockNumber) {}

    /// Returns the chunk root given `block_number` and `extrinsic_index`.
    pub fn chunk_root(block_number: T::BlockNumber, extrinsic_index: u32) -> Option<T::Hash> {
        <ChunkRootIndex<T>>::get((block_number, extrinsic_index))
    }

    /// Returns the block number in which the recall byte is included.
    pub fn find_recall_block(recall_byte: u64) -> Option<T::BlockNumber> {
        frame_support::log::debug!(
            target: "runtime::permastore",
            "Global weave size list: {:?}",
            <GlobalBlockNumberIndex<T>>::get()
                .iter()
                .zip(<GlobalWeaveSizeIndex<T>>::get().iter())
                .collect::<Vec<_>>()
        );
        let weave_size_list = <GlobalWeaveSizeIndex<T>>::get();

        let recall_block_number_index =
            match weave_size_list.binary_search_by_key(&recall_byte, |&weave_size| weave_size) {
                Ok(i) => i,
                Err(i) => i,
            };

        frame_support::log::debug!(
            target: "runtime::permastore",
            "Found the index of recall block number: {}",
            recall_block_number_index,
        );

        <GlobalBlockNumberIndex<T>>::get()
            .get(recall_block_number_index)
            .copied()
    }

    /// Returns the data size of transaction given `block_number` and `extrinsic_index`.
    pub fn data_size(block_number: T::BlockNumber, extrinsic_index: u32) -> u32 {
        <TransactionDataSize<T>>::get((block_number, extrinsic_index))
    }

    /// Returns the data size of current block.
    pub fn block_size() -> u64 {
        <BlockDataSize<T>>::get()
    }

    /// Returns the total byte size of weave.
    pub fn weave_size() -> u64 {
        <WeaveSize<T>>::get()
    }
}

/// A signed extension that checks for the `store` call.
///
/// It ensures the transaction data has been stored locally.
#[derive(Encode, Decode, Clone, Eq, PartialEq)]
pub struct CheckStore<T: Config + Send + Sync>(PhantomData<T>);

impl<T: Config + Send + Sync> sp_std::fmt::Debug for CheckStore<T> {
    fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        write!(f, "CheckStore")
    }
}

impl<T: Config + Send + Sync> SignedExtension for CheckStore<T>
where
    <T as frame_system::Config>::Call: IsSubType<Call<T>>,
{
    const IDENTIFIER: &'static str = "CheckStore";
    type AccountId = T::AccountId;
    type Call = <T as frame_system::Config>::Call;
    type AdditionalSigned = ();
    type Pre = ();

    fn additional_signed(&self) -> sp_std::result::Result<(), TransactionValidityError> {
        Ok(())
    }

    fn validate(
        &self,
        who: &Self::AccountId,
        call: &Self::Call,
        _info: &DispatchInfoOf<Self::Call>,
        _len: usize,
    ) -> TransactionValidity {
        if let Some(Call::store(data_size, chunk_root, ..)) = call.is_sub_type() {
            // TODO:
            //
            // 1. Check the balance is enough to pay the storage fee according to the data size.
            //
            // 2. Check if the data has been stored locally.
            //
            // 3. Adjust the transaction priority according to the data size.

            ensure!(
                T::Currency::free_balance(who) >= Pallet::<T>::calculate_storage_fee(*data_size),
                InvalidTransaction::Payment
            );

            const DATA_NOT_STORED: u8 = 100;
            ensure!(
                Pallet::<T>::stored_locally(chunk_root),
                InvalidTransaction::Custom(DATA_NOT_STORED)
            );
        }

        Ok(Default::default())
    }
}
