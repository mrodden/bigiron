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

use std::path::{Path, PathBuf};

use hex;
use serde_yaml;
use sha2::{Digest, Sha256};
use tracing::error;
use url::Url;

use crate::dnsmasq::Dnsmasq;
use crate::error::Error;
use crate::imagerepo::ImageRepo;
use crate::libvirt;
use crate::models;
use crate::models::to_size;
use crate::network;

mod imgutil {
    use std::path::Path;
    use std::process::Command;

    use tracing::debug;

    use crate::error::Error;

    pub fn create<P: AsRef<Path>, B: AsRef<Path>>(
        filepath: P,
        resize: Option<u64>,
        backing_file: Option<B>,
    ) -> Result<(), Error> {
        let mut cmd = Command::new("/usr/bin/qemu-img");
        cmd.arg("create");
        cmd.arg("-q");

        if let Some(bf) = backing_file {
            cmd.arg("-b");
            cmd.arg(bf.as_ref());
        }

        cmd.arg("-f");
        cmd.arg("qcow2");
        cmd.arg(filepath.as_ref());

        if let Some(size) = resize {
            cmd.arg(size.to_string());
        }

        debug!("Running: {:?}", cmd);
        let r = cmd.status()?;
        if r.success() {
            return Ok(());
        } else {
            return Err("failed to create new image".into());
        }
    }
}

pub fn apply_specfile<P: AsRef<Path>>(path: P) -> Result<(), Error> {
    let store = Store::new();

    let buf = std::fs::read_to_string(path.as_ref())?;

    let docs: Vec<&str> = buf.split("---").collect();

    for (i, doc) in docs.iter().enumerate() {
        if doc.len() > 0 {
            let r = match serde_yaml::from_str::<models::Resource>(&doc) {
                Ok(r) => r,
                Err(e) => {
                    return Err(format!("Error reading document at index {}: {}", i, e).into())
                }
            };

            match r {
                models::Resource::Machine(mut m) => {
                    if store.get_machine(&m.name).is_none() {
                        store.add_machine(&m)?;
                        if create_machine(&mut m).is_err() {
                            store.remove_machine(&m.name)?;
                            eprintln!("Failed to create VM: {}", &m.name);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn create_machine(machine: &mut models::Machine) -> Result<(), Error> {
    // resolve image
    let images = ImageRepo::new();

    let image_url = Url::parse(&machine.spec.image.url)?;
    let image = images.add_from_url(image_url).unwrap();

    let s = Store::new();

    // create derived image file in data dir
    let imgpath = s.path_for_machine(&machine.name).join("image.qcow2");
    imgutil::create(
        &imgpath,
        machine
            .spec
            .image
            .resize
            .as_ref()
            .map(|s| to_size(s).expect("error parsing size value")),
        Some(image.path),
    )?;

    // create additional storage drives in data dir
    // FIXME(mrodden): implement me

    // ensure bridged management network
    // FIXME(mrodden): implement me
    let bridge_name = "br0";

    // generate MAC and IP
    let netinfo = network::new_reservation(&machine.name);
    let dnsmasq = Dnsmasq::new();
    dnsmasq.add_host(&netinfo.mac, &netinfo.ip, &netinfo.hostname);

    // create libvirt XML definition
    // create domain from XML definition
    // start VM
    libvirt::define(&machine, &imgpath, &bridge_name, &netinfo.mac)?;

    Ok(())
}

pub fn get_machine_by_id(id: &str) -> Option<models::Machine> {
    let store = Store::new();
    store.get_machine(id)
}

pub fn delete_machine(id: &str) -> Result<(), Error> {
    let store = Store::new();
    if store.get_machine(id).is_some() {
        if let Err(e) = libvirt::destroy(id) {
            return Err(format!("Error while shutting down libvirt domain='{}': {}", id, e).into());
        }
    }
    if let Err(err) = network::remove_reservation(&id) {
        error!("error while removing network reservation: {}", err);
    }
    let dnsmasq = Dnsmasq::new();
    dnsmasq.rm_host(&id);
    store.remove_machine(id)?;
    Ok(())
}

fn get_unique_id(name: &str) -> String {
    let mut h = Sha256::new();
    h.update(name.as_bytes());
    let r = h.finalize();
    let mut h = hex::encode(r);

    // truncate to 128 bits (32 chars) so we can interoperate with UUIDs.
    // this does not affect the uniqueness enough to be an issue here.
    // obviously, do not do this if used for crypto
    h.truncate(32);
    h
}

pub struct Store {
    path: PathBuf,
}

fn machine_from_file<P: AsRef<Path>>(path: P) -> models::Machine {
    let buf = std::fs::read_to_string(path.as_ref()).expect("error reading spec file");
    let m =
        serde_yaml::from_str::<models::Machine>(&buf).expect("error parsing spec file contents");
    m
}

impl Store {
    pub fn new() -> Self {
        let path = Path::new("/var/lib/bigiron/libvirt");

        if !path.exists() {
            std::fs::create_dir_all(path).expect("error creating datastore directory");
        }
        Self {
            path: path.to_path_buf(),
        }
    }

    pub fn get_machine(&self, id: &str) -> Option<models::Machine> {
        let mp = self.path.join(get_unique_id(id));

        if !mp.exists() {
            return None;
        }

        let sp = mp.join("spec.yaml");
        let m = machine_from_file(&sp);
        Some(m)
    }

    pub fn path_for_machine(&self, id: &str) -> PathBuf {
        self.path.join(get_unique_id(id))
    }

    pub fn list_machines(&self) -> Vec<models::Machine> {
        let mut r = Vec::new();
        for e in self
            .path
            .read_dir()
            .expect("error reading data store directories")
        {
            if let Ok(entry) = e {
                let m = machine_from_file(&entry.path().join("spec.yaml"));
                r.push(m);
            }
        }
        r
    }

    pub fn add_machine(&self, machine: &models::Machine) -> Result<(), Error> {
        if self.get_machine(&machine.name).is_some() {
            return Err("Machine with name already exists".into());
        }

        let mp = self.path.join(get_unique_id(&machine.name));
        std::fs::create_dir_all(&mp).expect("error creating machine diretory");

        let sp = mp.join("spec.yaml");
        let buf = serde_yaml::to_string(machine).expect("error serializing machine spec");
        std::fs::write(sp, buf.as_bytes()).expect("error writing spec file");

        Ok(())
    }

    pub fn remove_machine(&self, id: &str) -> Result<(), Error> {
        if self.get_machine(id).is_none() {
            return Err(format!("No machine with id='{}'", id).into());
        }

        let mp = self.path.join(get_unique_id(id));
        std::fs::remove_dir_all(mp)?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_get_unique_id() {
        let name = "test1234";
        let id = get_unique_id(name);
        assert_eq!(id, "937e8d5fbb48bd4949536cd65b8d35c4");

        let name = "test1324";
        let id = get_unique_id(name);
        assert_eq!(id, "9884aab1d7385f53a0e96bac13b6d7b5");
    }
}
