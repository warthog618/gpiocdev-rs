// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use clap::Parser;

mod common;
mod detect;
mod find;
mod get;
mod info;
mod monitor;
mod set;
mod watch;

fn main() {
    let opt = Opts::parse();
    let res = match opt.cmd {
        Command::Detect(cfg) => detect::cmd(&cfg),
        Command::Find(cfg) => find::cmd(&cfg),
        Command::Get(cfg) => get::cmd(&cfg),
        Command::Info(cfg) => info::cmd(&cfg),
        Command::Monitor(cfg) => monitor::cmd(&cfg),
        Command::Set(cfg) => set::cmd(&cfg),
        Command::Watch(cfg) => watch::cmd(&cfg),
    };
    if let Err(e) = res {
        if opt.verbose {
            eprintln!("{:?}", e)
        } else {
            eprintln!("{}", e)
        }
        std::process::exit(1);
    }
}

#[derive(Parser)]
#[clap(
    name = "gpiodctl",
    about = "A utility to control GPIO lines on Linux GPIO character devices."
)]
struct Opts {
    /// Provide more detailed error messages.
    #[clap(short = 'v', long)]
    pub verbose: bool,

    #[clap(subcommand)]
    cmd: Command,
}

#[derive(Parser)]
enum Command {
    /// Get info for the GPIO chips present on the system.
    Detect(detect::Opts),
    /// Find a line by name.
    Find(find::Opts),
    /// Read the values of a set of lines.
    Get(get::Opts),
    /// Get information for lines.
    Info(info::Opts),
    /// Monitor changes in line values.
    Monitor(monitor::Opts),
    /// Set the value of a set of lines.
    Set(set::Opts),
    /// Watch changes to line request and configuration state.
    Watch(watch::Opts),
}
