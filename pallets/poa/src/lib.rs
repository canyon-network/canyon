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

//! Proof of Access consensus.
//!
//! Records the storage capacity of each validator on chain.

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(unused)]

use sp_runtime::{
    traits::{Bounded, DispatchInfoOf, SaturatedConversion, SignedExtension},
    transaction_validity::{
        InvalidTransaction, TransactionValidity, TransactionValidityError, ValidTransaction,
    },
};
use sp_std::{marker::PhantomData, prelude::*};

use frame_support::{
    inherent::{InherentData, InherentIdentifier, MakeFatalError, ProvideInherent},
    traits::IsSubType,
    weights::{ClassifyDispatch, DispatchClass, Pays, PaysFee, WeighData, Weight},
};
use frame_system::ensure_signed;

use canyon_primitives::Depth;

#[cfg(any(feature = "runtime-benchmarks", test))]
mod benchmarking;
#[cfg(all(feature = "std", test))]
mod mock;
#[cfg(all(feature = "std", test))]
mod tests;

/// A type alias for the balance type from this pallet's point of view.
type BalanceOf<T> = <T as pallet_balances::Config>::Balance;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    /// Our pallet's configuration trait. All our types and constants go in here. If the
    /// pallet is dependent on specific other pallets, then their configuration traits
    /// should be added to our implied traits list.
    ///
    /// `frame_system::Config` should always be included.
    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_balances::Config {
        /// The overarching event type.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Update the SLA of validator.
        #[pallet::weight(0)]
        pub fn update_storage_capacity(
            origin: OriginFor<T>,
            #[pallet::compact] depth: Depth,
        ) -> DispatchResult {
            ensure_root(origin)?;

            Ok(())
        }
    }

    /// Event for the poa pallet.
    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Dummy event, just here so there's a generic type that's used.
        NewDepth(T::AccountId, Depth),
    }

    /// Error for the poa pallet.
    #[pallet::error]
    pub enum Error<T> {
        /// poa inherent is required on each valid block.
        MandatoryInherentMissing,
    }

    /// The estimate of the proportion of validator's local storage to
    /// the entire network storage.
    ///
    /// Indicated by the average depth of poa generation of a validator.
    /// The smaller the depth, the greater the capacity. The smallest depth
    /// is 1, which means the validator stores the entire weave locally.
    #[pallet::storage]
    #[pallet::getter(fn capacity_estimation)]
    pub(super) type CapacityEstimation<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Depth>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub validator_initial_capacity: Vec<(T::AccountId, Depth)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                validator_initial_capacity: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            for (validator, depth) in &self.validator_initial_capacity {
                <CapacityEstimation<T>>::insert(validator, depth);
            }
        }
    }
}

impl<T: Config> ProvideInherent for Pallet<T> {
    type Call = Call<T>;
    type Error = MakeFatalError<()>;

    const INHERENT_IDENTIFIER: InherentIdentifier = canyon_primitives::POA_INHERENT_IDENTIFIER;

    fn create_inherent(data: &InherentData) -> Option<Self::Call> {
        frame_support::log::info!(
            "poa inherent: {:?}",
            data.get_data::<Option<u32>>(&Self::INHERENT_IDENTIFIER)
        );
        let depth: Option<u32> = match data.get_data(&Self::INHERENT_IDENTIFIER) {
            Ok(Some(d)) => d,
            Ok(None) => return None,
            Err(_) => {
                frame_support::log::error!("Depth failed to decode");
                return None;
            }
        };

        if let Some(depth) = depth {
            Some(Call::update_storage_capacity(depth))
        } else {
            None
        }
    }

    fn is_inherent(call: &Self::Call) -> bool {
        matches!(call, Call::update_storage_capacity(..))
    }

    /// Required when inherent data is Some(_).
    ///
    /// NOTE: inherent data can only be None when the weave is empty.
    fn is_inherent_required(data: &InherentData) -> Result<Option<Self::Error>, Self::Error> {
        match data.get_data::<Option<u32>>(&Self::INHERENT_IDENTIFIER) {
            Ok(Some(_d)) => Ok(Some(().into())),
            _ => Ok(None),
        }
    }
}
