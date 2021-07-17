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
//! depth info from PoA consensus engine of validators on chain, which
//! is used to estimate the actual storage capacity of a validator.
//!
//! we can say a validator stores 100% of the network data locally if
//! it has produced N blocks with a total depth of N. Furthermore, the
//! estimated result becomes increasingly accurate and reliable with
//! more and more blocks being authored by that validator.
//!
//! ## Interface
//!
//! ### Inherent Extrinsics
//!
//! The Poa pallet creates the [`note_depth`] inherent when the data for
//! [`POA_INHERENT_IDENTIFIER`] is Some(_) and decodable.

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};

use sp_runtime::{
    generic::DigestItem,
    traits::{AtLeast32BitUnsigned, SaturatedConversion},
    Permill,
};
use sp_std::prelude::*;

use frame_support::{
    inherent::{InherentData, InherentIdentifier, MakeFatalError, ProvideInherent},
    RuntimeDebug,
};

use canyon_primitives::Depth;
use cp_consensus_poa::{PoaOutcome, POA_ENGINE_ID, POA_INHERENT_IDENTIFIER};

// #[cfg(any(feature = "runtime-benchmarks", test))]
// mod benchmarking;
#[cfg(all(feature = "std", test))]
mod mock;
#[cfg(all(feature = "std", test))]
mod tests;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

/// Historical info about the average value of depth.
///
/// This struct is used for calculating the historical average depth
/// of a validator, which implies the storage capacity per validator.
#[derive(RuntimeDebug, Clone, Eq, PartialEq, Encode, Decode)]
pub struct DepthInfo<BlockNumber> {
    /// Sum of total depth so far.
    pub total_depth: Depth,
    /// Number of blocks authored by a validator since the weave is non-empty.
    ///
    /// The `blocks` here is not equal to the number of total blocks
    /// authored by a validator since genesis block because the poa
    /// contruction is skipped when the weave is empty, the blocks
    /// authored in that period are not counted.
    pub blocks: BlockNumber,
}

impl<BlockNumber: AtLeast32BitUnsigned + Copy> DepthInfo<BlockNumber> {
    /// Adds a new depth to the historical depth info.
    ///
    /// # NOTE
    ///
    /// `depth` has been ensured to be greater than 0 when creating inherent.
    /// The smallest depth is 1, which means the block author located the
    /// recall block at the first time.
    pub fn add_depth(&mut self, depth: Depth) {
        self.total_depth += depth;
        self.blocks += 1u32.into();
    }

    /// Returns the calculated storage capacity given historical depth info.
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
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(_n: BlockNumberFor<T>) {}
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(0)]
        pub fn process_poa_outcome(
            origin: OriginFor<T>,
            poa_outcome: PoaOutcome,
        ) -> DispatchResult {
            ensure_none(origin)?;

            match poa_outcome {
                PoaOutcome::Justification(poa) => {
                    let depth = poa.depth;

                    assert!(depth > 0, "depth must be greater than 0");

                    <frame_system::Pallet<T>>::deposit_log(
                        DigestItem::Seal(POA_ENGINE_ID, poa.encode()).into(),
                    );

                    Self::note_depth(depth);
                }
                PoaOutcome::MaxDepthReached => {
                    // Decrease the storage capacity?
                    // Need to update outcome.require_inherent() too.
                    //
                    // TODO: slash the block author when SLA is too low?
                    // None
                }
                // PoaOutcome::Skipped => None,
                PoaOutcome::Skipped => (),
            }

            Ok(())
        }
    }

    #[pallet::inherent]
    impl<T: Config> ProvideInherent for Pallet<T> {
        type Call = Call<T>;
        type Error = MakeFatalError<()>;

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
            Some(Call::process_poa_outcome(poa_outcome))
        }

        fn is_inherent(call: &Self::Call) -> bool {
            matches!(call, Call::process_poa_outcome(..))
        }

        fn is_inherent_required(data: &InherentData) -> Result<Option<Self::Error>, Self::Error> {
            match data.get_data::<PoaOutcome>(&Self::INHERENT_IDENTIFIER) {
                Ok(Some(outcome)) if outcome.require_inherent() => Ok(Some(().into())),
                _ => Ok(None),
            }
        }
    }

    /// Event for the poa pallet.
    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId")]
    pub enum Event<T: Config> {
        /// Dummy event, just here so there's a generic type that's used.
        NewDepth(T::AccountId, Depth),
    }

    /// Error for the poa pallet.
    #[pallet::error]
    pub enum Error<T> {
        /// Poa inherent is required but there is no one.
        PoaInherentMissing,
    }

    /// Historical depth info for each validator.
    ///
    /// The probabilistic estimate of the proportion of each
    /// validator's local storage to the entire network storage.
    ///
    /// Indicated by the average depth of poa generation of a validator.
    /// The smaller the depth, the greater the storage capacity.
    #[pallet::storage]
    #[pallet::getter(fn history_depth)]
    pub(super) type HistoryDepth<T: Config> =
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
                        total_depth: depth,
                        blocks: 1u32.into(),
                    },
                );
            }
        }
    }
}
