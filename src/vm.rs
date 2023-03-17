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

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use libc;
use serde::{Deserialize, Serialize};
use serde_json;
use tracing::warn;
use uuid::Uuid;

use crate::error::Error;

#[derive(Debug, Clone)]
pub struct VMSet {
    path: PathBuf,
}

impl VMSet {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let rpath = path.as_ref();

        if !rpath.exists() {
            std::fs::create_dir_all(&rpath).expect("error creating vmset directory");
        }
        Self {
            path: rpath.to_path_buf(),
        }
    }

    pub fn list_vms(&self) -> Vec<String> {
        self.path
            .read_dir()
            .expect("error reading VMSet path")
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect()
    }

    pub fn get(&self, id: &str) -> Result<VM, Error> {
        let vmpath = self.path.join(&id);
        let specpath = vmpath.join("spec.json");

        if !vmpath.exists() || !specpath.exists() {
            return Err(format!("No VM for id={} found", id).into());
        }

        let r = File::open(&specpath)?;
        let mut vm: VM = serde_json::from_reader(&r)?;
        vm.path = vmpath;

        Ok(vm)
    }

    pub fn define(&self, spec: Spec) -> VM {
        let id = Uuid::new_v4().to_string();
        let path = self.path.join(&id);
        let vm = VM { id, spec, path };

        std::fs::create_dir_all(&vm.path()).expect("error creating vm directory");

        let mut f = File::options()
            .create(true)
            .write(true)
            .open(&vm.spec_path())
            .unwrap();
        write!(f, "{}", serde_json::to_string(&vm).unwrap()).unwrap();

        vm
    }
}

impl Default for VMSet {
    fn default() -> Self {
        let uid = unsafe { libc::getuid() };
        let mut path = PathBuf::new();
        if uid != 0 {
            path.push(&std::env::var("HOME").unwrap());
            path.push(".config/bigiron");
        } else {
            path.push("/var/lib/bigiron");
        }
        Self::new(path)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spec {
    pub name: String,
    pub cpus: u32,
    pub memory_mb: u64,
    pub image: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VM {
    id: String,
    spec: Spec,

    #[serde(skip)]
    path: PathBuf,
}

use crate::qemu;

impl VM {
    pub fn start(&self) -> Result<(), Error> {
        if self.running() {
            return Err("VM already started".into());
        }

        let p = qemu::Process::new(
            &self.path,
            &self.spec.name,
            self.spec.cpus,
            self.spec.memory_mb,
            &self.id,
            qemu::Image {
                path: self.spec.image.clone(),
            },
        );

        p.launch();
        Ok(())
    }

    pub fn running(&self) -> bool {
        if !self.path().join("monitor.sock").exists() {
            return false;
        }

        if !self.path().join("pid").exists() {
            return false;
        }

        let mut pfile = File::open(self.path().join("pid")).expect("error opening PID file");
        let mut pst = String::new();
        let _ = pfile.read_to_string(&mut pst).unwrap();
        let pid = pst.parse::<u32>().expect("error parsing PID file contents");

        if !PathBuf::new().join(format!("/proc/{}", pid)).exists() {
            return false;
        }

        true
    }

    fn monitor(&self) -> Result<qemu::Monitor, Error> {
        if !self.running() {
            return Err("VM not started".into());
        }
        let monp = self.path.join("monitor.sock");
        qemu::Monitor::connect(monp)
    }

    pub fn destroy(&self) -> Result<(), Error> {
        self.monitor()?.quit()
    }

    pub fn stop(&self) -> Result<(), Error> {
        self.monitor()?.stop()
    }

    pub fn cont(&self) -> Result<(), Error> {
        self.monitor()?.cont()
    }

    pub fn status(&self) -> Result<String, Error> {
        return self.monitor()?.status();
    }

    pub fn id(&self) -> String {
        self.id.clone()
    }

    pub fn name(&self) -> String {
        self.spec.name.clone()
    }

    pub fn pid(&self) -> Option<u32> {
        match File::open(self.path().join("pid")) {
            Err(e) => {
                // pid file most likely doesn't exist
                warn!("error opening pid file: {}", e);
                return None;
            }
            Ok(mut f) => {
                let mut buf = String::new();
                f.read_to_string(&mut buf)
                    .expect("error reading from pid file");
                let pid: u32 = buf.parse().expect("error parsing pid from file");
                return Some(pid);
            }
        }
    }

    fn path(&self) -> PathBuf {
        self.path.clone()
    }

    fn spec_path(&self) -> PathBuf {
        self.path().join("spec.json")
    }

    pub fn undefine(self) -> Result<(), Error> {
        if self.running() {
            self.destroy()?;
        }

        std::fs::remove_dir_all(self.path()).unwrap();

        Ok(())
    }
}
