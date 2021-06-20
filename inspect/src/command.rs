// This file is part of Canyon.

// Copyright (C) 2021 Canyon Network.
// License: GPL-3.0

//! Command ran by the CLI

use std::str::FromStr;

use sc_cli::{CliConfiguration, ImportParams, Result, SharedParams};
use sc_service::{new_full_client, Configuration, NativeExecutionDispatch};
use sp_runtime::traits::Block;

use crate::cli::{InspectCmd, InspectSubCmd};
use crate::Inspector;

impl InspectCmd {
    /// Run the inspect command, passing the inspector.
    pub fn run<B, RA, EX>(&self, config: Configuration) -> Result<()>
    where
        B: Block,
        B::Hash: FromStr,
        RA: Send + Sync + 'static,
        EX: NativeExecutionDispatch + 'static,
    {
        let client = new_full_client::<B, RA, EX>(&config, None)?;
        let inspect = Inspector::<B>::new(client);

        match &self.command {
            InspectSubCmd::Block { input } => {
                let input = input.parse()?;
                let res = inspect.block(input).map_err(|e| format!("{}", e))?;
                println!("{}", res);
                Ok(())
            }
            InspectSubCmd::Extrinsic { input } => {
                let input = input.parse()?;
                let res = inspect.extrinsic(input).map_err(|e| format!("{}", e))?;
                println!("{}", res);
                Ok(())
            }
        }
    }
}

impl CliConfiguration for InspectCmd {
    fn shared_params(&self) -> &SharedParams {
        &self.shared_params
    }

    fn import_params(&self) -> Option<&ImportParams> {
        Some(&self.import_params)
    }
}
