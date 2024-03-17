// This file is part of Canyon.

// Copyright (C) 2021 Canyon Network.
// License: GPL-3.0

//! Structs to easily compose inspect sub-command for CLI.

use sc_cli::{ImportParams, SharedParams};

/// The `inspect` command used to print decoded chain data.
#[derive(Debug, clap::Parser)]
pub struct InspectCmd {
    #[allow(missing_docs)]
    #[clap(subcommand)]
    pub command: InspectSubCmd,

    #[allow(missing_docs)]
    #[clap(flatten)]
    pub shared_params: SharedParams,

    #[allow(missing_docs)]
    #[clap(flatten)]
    pub import_params: ImportParams,
}

/// A possible inspect sub-commands.
#[derive(Debug, clap::Subcommand)]
pub enum InspectSubCmd {
    /// Decode block with native version of runtime and print out the details.
    Block {
        /// Address of the block to print out.
        ///
        /// Can be either a block hash (no 0x prefix) or a number to retrieve existing block,
        /// or a 0x-prefixed bytes hex string, representing SCALE encoding of
        /// a block.
        #[arg(value_name = "HASH or NUMBER or BYTES")]
        input: String,
    },
    /// Decode extrinsic with native version of runtime and print out the details.
    Extrinsic {
        /// Address of an extrinsic to print out.
        ///
        /// Can be either a block hash (no 0x prefix) or number and the index, in the form
        /// of `{block}:{index}` or a 0x-prefixed bytes hex string,
        /// representing SCALE encoding of an extrinsic.
        #[arg(value_name = "BLOCK:INDEX or BYTES")]
        input: String,
    },
}
