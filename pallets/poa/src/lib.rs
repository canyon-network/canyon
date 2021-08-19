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

//! # Poa Pallet
//!
//! The Poa pallet provides the feature of recording the validators'
//! historical depth info from PoA consensus engine on chain, which
//! can be used to estimate the actual storage capacity of a validator.
//!
//! we can say a validator tends to have stored 100% of the network
//! data locally with a great chance if it had produced N blocks with
//! a total depth of N.  The estimated result becomes increasingly
//! accurate and reliable with more and more blocks being authored
//! by that validator.
//!
//! ## Interface
//!
//! ### Inherent Extrinsics
//!
//! The Poa pallet creates the inherent extrinsic [`Call::deposit`]
//! when the inherent data contains a valid [`POA_INHERENT_IDENTIFIER`].

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]
#![deny(rustdoc::broken_intra_doc_links)]

use codec::{Decode, Encode, MaxEncodedLen};

use sp_runtime::{
    generic::DigestItem,
    traits::{AtLeast32BitUnsigned, SaturatedConversion},
    Permill,
};
use sp_std::prelude::*;

use frame_support::{
    inherent::{InherentData, InherentIdentifier, MakeFatalError, ProvideInherent},
    weights::DispatchClass,
    RuntimeDebug,
};

use canyon_primitives::Depth;
use cp_consensus_poa::{PoaConfiguration, PoaOutcome, POA_ENGINE_ID, POA_INHERENT_IDENTIFIER};

#[cfg(any(feature = "runtime-benchmarks", test))]
mod benchmarking;
#[cfg(all(feature = "std", test))]
mod mock;
#[cfg(all(feature = "std", test))]
mod tests;
pub mod weights;

pub use self::weights::WeightInfo;
// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

/// Historical info about the average value of depth.
///
/// This struct is used for calculating the historical average depth
/// of a validator, which implies the storage capacity per validator.
#[derive(RuntimeDebug, Clone, Eq, PartialEq, Encode, Decode, MaxEncodedLen)]
pub struct DepthInfo<BlockNumber> {
    /// Number of blocks authored by a validator since the weave is non-empty.
    ///
    /// The `blocks` here is not equal to the number of total blocks
    /// authored by a validator from the genesis block because the poa
    /// contruction is skipped when the weave is empty, the blocks
    /// authored in that period are not counted.
    pub blocks: BlockNumber,
    /// Sum of all depths so far.
    pub total_depth: Depth,
}

impl<BlockNumber: AtLeast32BitUnsigned + Copy> DepthInfo<BlockNumber> {
    /// Adds a new depth to the historical depth info.
    ///
    /// # NOTE
    ///
    /// The smallest depth is 1, which has been ensured when creating
    /// the inherent, it means the block author located the recall
    /// block at the first time.
    pub fn add_depth(&mut self, depth: Depth) {
        self.total_depth += depth;
        self.blocks += 1u32.into();
    }

    /// Returns the calculated storage capacity.
    ///
    /// In theory, the greater the historical average depth, the less the
    /// storage of node stored locally.
    ///
    /// ```text
    ///    average_depth = self.total_depth / self.blocks
    ///
    /// storage_capacity = 1 / average_depth
    ///                  = self.blocks / self.total_depth
    /// ```
    pub fn as_storage_capacity(&self) -> Permill {
        Permill::from_rational(self.blocks, self.total_depth.saturated_into())
    }
}

/// Trait for providing the author of current block.
pub trait BlockAuthor<AccountId> {
    /// Returns the author of current building block.
    fn author() -> AccountId;
}

/// Error type for the poa inherent.
#[derive(RuntimeDebug, Clone, Encode, Decode)]
pub enum InherentError {
    /// The poa entry included is invalid.
    InvalidProofOfAccess,
    /// Poa inherent is not provided.
    MissingPoaInherent,
}

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
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// Find the author of current block.
        type BlockAuthor: BlockAuthor<Self::AccountId>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::generate_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(_n: BlockNumberFor<T>) {}
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Handle the inherent data from the poa consensus.
        ///
        /// Deposit a consensus log if `poa_outcome` is valid.
        #[pallet::weight((T::WeightInfo::deposit(), DispatchClass::Mandatory))]
        pub fn deposit(origin: OriginFor<T>, poa_outcome: PoaOutcome) -> DispatchResult {
            ensure_none(origin)?;

            match poa_outcome {
                PoaOutcome::Justification(poa) => {
                    poa.check_validity(&Self::poa_config()).map_err(|e| {
                        frame_support::log::error!(
                            target: "runtime::poa",
                            "Checking poa validity failed when creating the poa `deposit` inherent: {:?}",
                            e,
                        );
                        Error::<T>::InvalidProofOfAccess
                    })?;

                    Self::note_depth(poa.depth);
                    <frame_system::Pallet<T>>::deposit_log(DigestItem::Seal(
                        POA_ENGINE_ID,
                        poa.encode(),
                    ));
                }
                PoaOutcome::MaxDepthReached(_) => {
                    // Decrease the storage capacity?
                    // Need to update outcome.require_inherent() too.
                    //
                    // TODO: slash the block author when SLA is too low?
                    // None
                }
                PoaOutcome::Skipped => (),
            }

            Ok(())
        }

        /// Set new poa configuration.
        #[pallet::weight(T::WeightInfo::set_config())]
        pub fn set_config(origin: OriginFor<T>, new: PoaConfiguration) -> DispatchResult {
            ensure_root(origin)?;

            ensure!(new.check_sanity(), Error::<T>::InvalidPoaConfiguration);

            PoaConfig::<T>::put(&new);

            Self::deposit_event(Event::<T>::ConfigUpdated(new));

            Ok(())
        }
    }

    #[pallet::inherent]
    impl<T: Config> ProvideInherent for Pallet<T> {
        type Call = Call<T>;
        type Error = MakeFatalError<InherentError>;

        const INHERENT_IDENTIFIER: InherentIdentifier = POA_INHERENT_IDENTIFIER;

        fn create_inherent(data: &InherentData) -> Option<Self::Call> {
            let poa_outcome: PoaOutcome = match data.get_data(&Self::INHERENT_IDENTIFIER) {
                Ok(Some(outcome)) => outcome,
                Ok(None) => return None,
                Err(e) => {
                    frame_support::log::error!(
                        target: "runtime::poa",
                        "Error occurred when getting the inherent data of poa: {:?}",
                        e,
                    );
                    return None;
                }
            };

            // TODO: avoide double including the full ProofOfAccess struct in extrinsic
            // as it will be included in the header anyway?
            Some(Call::deposit(poa_outcome))
        }

        fn check_inherent(call: &Self::Call, _: &InherentData) -> Result<(), Self::Error> {
            match call {
                Call::deposit(PoaOutcome::Justification(poa)) => {
                    poa.check_validity(&Self::poa_config()).map_err(|e| {
                        frame_support::log::error!(
                            target: "runtime::poa",
                            "Check inherent failed due to poa is invalid: {:?}", e,
                        );
                        InherentError::InvalidProofOfAccess.into()
                    })
                }
                _ => Ok(()),
            }
        }

        fn is_inherent(call: &Self::Call) -> bool {
            matches!(call, Call::deposit(..))
        }

        fn is_inherent_required(data: &InherentData) -> Result<Option<Self::Error>, Self::Error> {
            match data.get_data::<PoaOutcome>(&Self::INHERENT_IDENTIFIER) {
                Ok(Some(outcome)) if outcome.require_inherent() => {
                    Ok(Some(InherentError::MissingPoaInherent.into()))
                }
                _ => Ok(None),
            }
        }
    }

    /// Event for the poa pallet.
    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// New poa configuration.
        ConfigUpdated(PoaConfiguration),
    }

    /// Error for the poa pallet.
    #[pallet::error]
    pub enum Error<T> {
        /// Invalid inherent data of `[ProofOfAccess]`
        InvalidProofOfAccess,
        /// The poa configuration failed the sanity checks.
        InvalidPoaConfiguration,
    }

    /// Poa Configuration.
    #[pallet::storage]
    #[pallet::getter(fn poa_config)]
    pub type PoaConfig<T: Config> = StorageValue<_, PoaConfiguration, ValueQuery>;

    /// Historical depth info for each validator.
    ///
    /// The probabilistic estimate of the proportion of each
    /// validator's local storage to the entire network storage.
    ///
    /// Indicated by the average depth of poa generation of a validator.
    /// The smaller the depth, the greater the storage capacity.
    #[pallet::storage]
    #[pallet::getter(fn history_depth)]
    pub type HistoryDepth<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, DepthInfo<T::BlockNumber>>;

    /// Helper storage item of current block author for easier testing.
    #[cfg(test)]
    #[pallet::storage]
    pub(super) type TestAuthor<T: Config> = StorageValue<_, T::AccountId, ValueQuery>;

    impl<T: Config> Pallet<T> {
        /// Updates the historical depth info of block author.
        pub(crate) fn note_depth(depth: Depth) {
            let block_author = T::BlockAuthor::author();

            if let Some(mut old) = HistoryDepth::<T>::get(&block_author) {
                old.add_depth(depth);
                HistoryDepth::<T>::insert(&block_author, old);
            } else {
                HistoryDepth::<T>::insert(
                    &block_author,
                    DepthInfo {
                        blocks: 1u32.into(),
                        total_depth: depth,
                    },
                );
            }
        }
    }
}
