// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: MIT

use clap::Parser;

mod common;
mod detect;
mod get;
mod info;
mod monitor;
mod set;
mod watch;

fn main() {
    let opt = Opts::parse();
    let res = match opt.cmd {
        Command::Detect(cfg) => detect::cmd(&cfg),
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
#[command(
    name = "gpiocdev",
    about = "A utility to control GPIO lines on Linux using GPIO character devices."
)]
struct Opts {
    /// Provide more detailed error messages.
    #[arg(short = 'v', long)]
    pub verbose: bool,

    #[command(subcommand)]
    cmd: Command,
}

#[derive(Parser)]
enum Command {
    /// Get info about the GPIO chips present on the system.
    Detect(detect::Opts),

    /// Read the values of lines.
    Get(get::Opts),

    /// Get information for lines.
    Info(info::Opts),

    /// Monitor lines for changes in value.
    Monitor(monitor::Opts),

    /// Set the value of lines.
    Set(set::Opts),

    /// Watch lines for requests and changes to configuration state.
    Watch(watch::Opts),
}
