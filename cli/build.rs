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

fn main() {
    #[cfg(feature = "cli")]
    cli::main();
}

#[cfg(feature = "cli")]
mod cli {
    include!("src/cli.rs");

    use clap::{CommandFactory, ValueEnum};
    use clap_complete::{generate_to, Shell};
    use std::{env, fs, path::Path};
    use substrate_build_script_utils::{generate_cargo_keys, rerun_if_git_head_changed};

    pub fn main() {
        build_shell_completion();
        generate_cargo_keys();

        rerun_if_git_head_changed();
    }

    /// Build shell completion scripts for all known shells
    /// Full list in https://github.com/kbknapp/clap-rs/blob/e9d0562a1dc5dfe731ed7c767e6cee0af08f0cf9/src/app/parser.rs#L123
    fn build_shell_completion() {
        for shell in Shell::value_variants() {
            build_completion(shell);
        }
    }

    /// Build the shell auto-completion for a given Shell
    fn build_completion(shell: &Shell) {
        let Some(outdir) = env::var_os("OUT_DIR") else {
            return;
        };
        let path = Path::new(&outdir)
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("completion-scripts");

        fs::create_dir(&path).ok();

        let _ = generate_to(*shell, &mut Cli::command(), "substrate-node", &path);
    }
}
