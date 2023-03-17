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

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tracing_subscriber;

use bigiron::api;
use bigiron::dnsmasq;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Apply {
        #[arg(required(true))]
        specfile: PathBuf,
    },
    List,
    Get {
        #[arg(required(true))]
        id: String,
    },
    Delete {
        #[arg(required(true))]
        id: String,
    },
    StartDhcp,
    StopDhcp,
    RestartDhcp,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match &cli.command {
        Commands::Apply { specfile } => {
            let _ = api::apply_specfile(specfile)?;
        }
        Commands::List => {
            let v = api::Store::new().list_machines();
            println!("{:-20} {:-10}", "NAME", "STATUS");
            for m in v {
                println!("{:-20} {:-10?}", m.name, m.status);
            }
        }
        Commands::Get { id } => match api::get_machine_by_id(&id) {
            Some(m) => println!("{}", m.to_yaml()?),
            None => println!("No machine found with id='{}'", id),
        },
        Commands::Delete { id } => {
            api::delete_machine(&id)?;
        }
        Commands::StartDhcp => {
            dnsmasq::Dnsmasq::new().start();
        }
        Commands::StopDhcp => {
            dnsmasq::Dnsmasq::new().stop();
        }
        Commands::RestartDhcp => {
            dnsmasq::Dnsmasq::new().stop();
            dnsmasq::Dnsmasq::new().start();
        }
    }

    Ok(())
}
