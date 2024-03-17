// This file is part of Canyon.

// Copyright (C) 2021 Canyon Network.
// License: GPL-3.0

//! Command ran by the CLI

use crate::cli::{InspectCmd, InspectSubCmd};
use crate::Inspector;
use sc_cli::{CliConfiguration, ImportParams, Result, SharedParams};
use sc_executor::NativeElseWasmExecutor;
use sc_service::{new_full_client, Configuration, NativeExecutionDispatch};
use sp_runtime::traits::Block;
use std::str::FromStr;

type HostFunctions = sp_io::SubstrateHostFunctions;

impl InspectCmd {
    /// Run the inspect command, passing the inspector.
    pub fn run<B, RA>(&self, config: Configuration) -> Result<()>
    where
        B: Block,
        RA: Send + Sync + 'static,
    {
        let executor = sc_service::new_wasm_executor::<HostFunctions>(&config);
        let client = new_full_client::<B, RA, _>(&config, None, executor)?;
        let inspect = Inspector::<B>::new(client);

        match &self.command {
            InspectSubCmd::Block { input } => {
                let input = input.parse()?;
                let res = inspect.block(input).map_err(|e| e.to_string())?;
                println!("{res}");
                Ok(())
            }
            InspectSubCmd::Extrinsic { input } => {
                let input = input.parse()?;
                let res = inspect.extrinsic(input).map_err(|e| e.to_string())?;
                println!("{res}");
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
