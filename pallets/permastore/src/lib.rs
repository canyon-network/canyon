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
    generic::{DataInfo, DigestItem},
    traits::{AccountIdConversion, DispatchInfoOf, SaturatedConversion, SignedExtension},
    transaction_validity::{InvalidTransaction, TransactionValidity, TransactionValidityError},
};
use sp_std::{marker::PhantomData, prelude::*};

use frame_support::{
    dispatch::DispatchResult,
    ensure,
    inherent::{InherentData, InherentIdentifier, MakeFatalError, ProvideInherent},
    traits::{Currency, ExistenceRequirement, Get, IsSubType},
};
use frame_system::ensure_signed;

use cp_consensus_poa::POA_ENGINE_ID;

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
        fn on_finalize(n: BlockNumberFor<T>) {
            let current_block_data_size = <BlockDataSize<T>>::take().unwrap_or_default();
            let last_weave_size = <WeaveSize<T>>::get().unwrap_or_default();
            let weave_size = last_weave_size + current_block_data_size;
            if current_block_data_size > 0 {
                <GlobalWeaveSizeList<T>>::append(weave_size);
                <GlobalBlockNumberIndex<T>>::append(n);
            }
            if weave_size > 0 {
                <frame_system::Pallet<T>>::deposit_log(DigestItem::Consensus(
                    POA_ENGINE_ID,
                    weave_size.encode(),
                ));
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Stores the data permanently.
        ///
        /// The minimum data size is 1 bytes, the maximum is `MAX_DATA_SIZE`.
        /// The digest of data will be recorded on chain, the actual data will
        /// be stored off-chain.
        #[pallet::weight(0)]
        pub fn store(
            origin: OriginFor<T>,
            data_size: u32,
            chunk_root: T::Hash,
            data: Vec<u8>, // store the data on chain for now
        ) -> DispatchResultWithPostInfo {
            let sender = ensure_signed(origin)?;

            ensure!(
                data_size > 0 && data_size < T::MaxDataSize::get(),
                Error::<T>::InvalidDataSize
            );
            ensure!(Self::stored_locally(&chunk_root), Error::<T>::NotStored);

            Self::charge_storage_fee(&sender, data_size)?;

            // Notes who stores the data.
            // who, (block_number, extrinsic_index) => data_info
            let block_number = frame_system::Pallet::<T>::block_number();
            let extrinsic_index = frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default();
            let data_info = DataInfo {
                size: data_size as u64,
                chunk_root,
            };
            Orders::<T>::insert(&sender, (block_number, extrinsic_index), Some(data_info));

            // FIXME: Move to off-chain solution
            PermaData::<T>::insert((block_number, extrinsic_index), data);

            let current_data_size = <BlockDataSize<T>>::get().unwrap_or_default();
            <BlockDataSize<T>>::put(current_data_size + data_size as u64);

            ChunkRootIndex::<T>::insert((block_number, extrinsic_index), chunk_root);

            Self::deposit_event(Event::Stored(sender, chunk_root));

            Ok(().into())
        }

        /// Forgets the data.
        #[pallet::weight(0)]
        pub fn forget(
            origin: OriginFor<T>,
            block_number: T::BlockNumber,
            extrinsic_index: ExtrinsicIndex,
        ) -> DispatchResultWithPostInfo {
            let sender = ensure_signed(origin)?;

            // Remove the order.
            let _data_info = Orders::<T>::take(&sender, (block_number, extrinsic_index))
                .ok_or(Error::<T>::OrderDoesNotExist)?;

            // refund the remaining fee.
            Self::refund_storage_fee(&sender, block_number);

            Self::deposit_event(Event::Forgot(block_number, extrinsic_index));

            Ok(().into())
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

    /// Map from all storage client to the info regarding the perma storage.
    #[pallet::storage]
    #[pallet::getter(fn ledger)]
    pub(super) type Ledger<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>>;

    /// Map of all the storage orders.
    #[pallet::storage]
    #[pallet::getter(fn orders)]
    pub(super) type Orders<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Twox64Concat,
        (T::BlockNumber, ExtrinsicIndex),
        Option<DataInfo<T::Hashing>>,
    >;

    /// FIXME: remove this once the offchain transaction data storage is done!
    /// Temeporary on-chain storage.
    #[pallet::storage]
    #[pallet::getter(fn perma_data)]
    pub(super) type PermaData<T: Config> =
        StorageMap<_, Blake2_128Concat, (T::BlockNumber, ExtrinsicIndex), Vec<u8>>;

    /// Total byte size of data stored onto network until last block.
    #[pallet::storage]
    #[pallet::getter(fn weave_size)]
    pub(super) type WeaveSize<T: Config> = StorageValue<_, u64>;

    /// Total byte size of data stored in current building block.
    #[pallet::storage]
    #[pallet::getter(fn block_data_size)]
    pub(super) type BlockDataSize<T: Config> = StorageValue<_, u64>;

    /// (block_number, extrinsic_index) => Option<chunk_root>
    #[pallet::storage]
    #[pallet::getter(fn chunk_root_index)]
    pub(super) type ChunkRootIndex<T: Config> =
        StorageMap<_, Twox64Concat, (T::BlockNumber, ExtrinsicIndex), T::Hash>;

    /// FIXME: find a proper way to store these info.
    ///
    /// Temp solution for locating the recall block. An ever increasing array of global weave size.
    #[pallet::storage]
    #[pallet::getter(fn global_block_size_index)]
    pub(super) type GlobalWeaveSizeList<T: Config> = StorageValue<_, Vec<u64>>;

    /// Temp solution for locating the recall block.
    #[pallet::storage]
    #[pallet::getter(fn global_block_number_index)]
    pub(super) type GlobalBlockNumberIndex<T: Config> = StorageValue<_, Vec<T::BlockNumber>>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub ledger: Vec<(T::AccountId, BalanceOf<T>)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                ledger: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            for (a, b) in &self.ledger {
                <Ledger<T>>::insert(a, b);
            }
        }
    }
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
    /// TODO: Currently all the fee is simply transfered to the treasury.
    fn charge_storage_fee(who: &T::AccountId, data_size: u32) -> DispatchResult {
        let fee = Self::calculate_storage_fee(data_size);
        let treasury_account: T::AccountId = T::TreasuryPalletId::get().into_account();
        T::Currency::transfer(who, &treasury_account, fee, ExistenceRequirement::KeepAlive)
    }

    fn refund_storage_fee(_who: &T::AccountId, _created_at: T::BlockNumber) {}

    /// Returns the chunk root given `block_number` and `extrinsic_index`.
    pub fn chunk_root(block_number: T::BlockNumber, extrinsic_index: u32) -> Option<T::Hash> {
        <ChunkRootIndex<T>>::get((block_number, extrinsic_index))
    }

    pub fn find_recall_block(recall_byte: u64) -> Option<T::BlockNumber> {
        let weave_size_list = <GlobalWeaveSizeList<T>>::get()?;

        let recall_block_number_index =
            match weave_size_list.binary_search_by_key(&recall_byte, |&weave_size| weave_size) {
                Ok(i) => i,
                Err(i) => i,
            };

        <GlobalBlockNumberIndex<T>>::get().map(|index| index[recall_block_number_index])
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

impl<T: Config> ProvideInherent for Pallet<T> {
    type Call = Call<T>;
    type Error = MakeFatalError<()>;

    const INHERENT_IDENTIFIER: InherentIdentifier =
        canyon_primitives::PERMASTORE_INHERENT_IDENTIFIER;

    fn create_inherent(data: &InherentData) -> Option<Self::Call> {
        let weave_size: u64 = match data.get_data(&Self::INHERENT_IDENTIFIER) {
            Ok(Some(d)) => d,
            Ok(None) => return None,
            Err(e) => {
                frame_support::log::error!(target: "runtime::permastore", "failed to decode weave size: {:?}", e);
                return None;
            }
        };

        if weave_size > 0 {
            <WeaveSize<T>>::put(weave_size);
            <frame_system::Pallet<T>>::deposit_log(DigestItem::PreRuntime(
                POA_ENGINE_ID,
                weave_size.encode(),
            ));
        }

        None
    }

    fn is_inherent(_call: &Self::Call) -> bool {
        false
    }
}
