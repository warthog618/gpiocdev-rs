// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use clap::Parser;
use std::process::ExitCode;

mod chip;
mod common;
mod edges;
mod get;
mod line;
mod notify;
mod set;

fn main() -> ExitCode {
    match Opts::try_parse() {
        Ok(opt) => {
            let res = match opt.cmd {
                Command::Chip(cfg) => chip::cmd(&cfg),
                Command::Edges(cfg) => edges::cmd(&cfg),
                Command::Get(cfg) => get::cmd(&cfg),
                Command::Line(cfg) => line::cmd(&cfg),
                Command::Set(cfg) => set::cmd(&cfg),
                Command::Notify(cfg) => notify::cmd(&cfg),
            };
            match res {
                Ok(()) => return ExitCode::SUCCESS,
                Err(e) if opt.verbose => eprintln!("{:#}", e),
                Err(e) => eprintln!("{}", e),
            }
        }
        Err(e) => eprintln!("{}", e),
    }
    ExitCode::FAILURE
}

#[derive(Parser)]
#[command(
    name = "gpiocdev",
    about = "A utility to control GPIO lines on Linux using GPIO character devices.",
    version,
    propagate_version = true
)]
struct Opts {
    /// Provide more detailed error messages.
    #[arg(short = 'v', long, global = true, display_order = 800)]
    pub verbose: bool,

    #[command(subcommand)]
    cmd: Command,
}

#[derive(Parser)]
enum Command {
    /// Get info about GPIO chips.
    Chip(chip::Opts),

    /// Monitor lines for edge events.
    Edges(edges::Opts),

    /// Read the values of lines.
    Get(get::Opts),

    /// Get information for lines.
    Line(line::Opts),

    /// Monitor lines for requests and changes to configuration state.
    Notify(notify::Opts),

    /// Set the value of lines.
    Set(set::Opts),
}
