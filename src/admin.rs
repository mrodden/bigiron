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

use bigiron::{vm, vm::VMSet};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    List,
    Define,
    Undefine {
        #[arg(required(true))]
        id: String,
    },
    Status {
        #[arg(required(true))]
        id: String,
    },
    Start {
        #[arg(required(true))]
        id: String,
    },
    Stop {
        #[arg(required(true))]
        id: String,
    },
    Cont {
        #[arg(required(true))]
        id: String,
    },
    Destroy {
        #[arg(required(true))]
        id: String,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match &cli.command {
        Commands::List => {
            println!("{0: <36}  {1: <30}", "ID", "NAME");
            let set = VMSet::default();
            for id in set.list_vms() {
                let vm = set.get(&id).unwrap();
                println!("{0: <36}  {1: <30}", vm.id(), vm.name());
            }
        }
        Commands::Define => {
            let c = VMSet::default();
            let vm = c.define(vm::Spec {
                name: "my-test-vm".into(),
                cpus: 2,
                memory_mb: 512,
                image: "image.qcow2".into(),
            });
            println!("VM Created\n{}", vm.id());
        }
        Commands::Undefine { id } => {
            let c = VMSet::default();
            let vm = c.get(&id).expect("no VM found");
            vm.undefine().unwrap();
        }
        Commands::Start { id } => {
            let c = VMSet::default();
            let vm = c.get(&id).expect("no VM found");
            vm.start()?;
        }
        Commands::Stop { id } => {
            let c = VMSet::default();
            let vm = c.get(&id).expect("no VM found");
            vm.stop()?;
            println!("{}", vm.status().unwrap());
        }
        Commands::Cont { id } => {
            let c = VMSet::default();
            let vm = c.get(&id).expect("no VM found");
            vm.cont()?;
            println!("{}", vm.status().unwrap());
        }
        Commands::Status { id } => {
            let c = VMSet::default();
            let vm = c.get(&id).expect("no VM found");
            println!("{}", vm.status().unwrap());
        }
        Commands::Destroy { id } => {
            let c = VMSet::default();
            let vm = c.get(&id).expect("no VM found");
            vm.destroy()?;
        }
    }

    Ok(())
}
