//  Copyright (C) 2022 IBM Corp.
//
//  This library is free software; you can redistribute it and/or
//  modify it under the terms of the GNU Lesser General Public
//  License as published by the Free Software Foundation; either
//  version 2.1 of the License, or (at your option) any later version.
//
//  This library is distributed in the hope that it will be useful,
//  but WITHOUT ANY WARRANTY; without even the implied warranty of
//  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
//  Lesser General Public License for more details.
//
//  You should have received a copy of the GNU Lesser General Public
//  License along with this library; if not, write to the Free Software
//  Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301
//  USA

use clap::{Parser, Subcommand};
use tracing_subscriber;

use bigiron::network;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Init,
    Add {
        mac: String,
        addr: String,
        hostname: Option<String>,
    },
    Old {
        mac: String,
        addr: String,
        hostname: Option<String>,
    },
    Del {
        mac: String,
        addr: String,
        hostname: Option<String>,
    },
}

fn main() {
    tracing_subscriber::fmt::init();

    eprintln!("{:?}", std::env::args_os());

    let cli = Cli::parse();
    eprintln!("{:?}", cli);

    match cli.command {
        Commands::Init => {}
        Commands::Add {
            mac,
            addr,
            hostname,
        } => {
            network::add_lease(&mac, &addr, hostname);
        }
        Commands::Old {
            mac,
            addr,
            hostname,
        } => {
            network::add_lease(&mac, &addr, hostname);
        }
        Commands::Del {
            mac,
            addr,
            hostname,
        } => {
            network::del_lease(&mac, &addr, hostname);
        }
    }
}
