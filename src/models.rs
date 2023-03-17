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

use serde::{Deserialize, Serialize};

use crate::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Resource {
    Machine(Machine),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Machine {
    pub name: String,
    pub status: Option<String>,
    pub spec: Spec,
}

impl Machine {
    pub fn to_yaml(&self) -> Result<String, Error> {
        let buf = serde_yaml::to_string(self)?;
        return Ok(buf);
    }
}

pub type SizeString = String;

pub fn to_size(s: &str) -> Result<u64, Error> {
    let mut last = &s[s.len() - 1..];
    let nlast = &s[s.len() - 2..s.len() - 1];
    let mut co: u64 = 1000;
    let mut num = &s[..s.len() - 1];

    if last == "i" {
        // binary byte mode
        co = 1024; 
        last = nlast;
        num = &s[..s.len() - 2];
    }

    let exp = match last {
        "T" | "t" => 3,
        "G" | "g" => 3,
        "M" | "m" => 2,
        "K" | "k" => 1,
        _ => 0,
    };

    let scalar = num.parse::<u64>()?;
    Ok(scalar * co.pow(exp))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spec {
    pub cpu: u32,
    pub memory: SizeString,
    pub image: Image,
    pub storage: Option<Vec<StorageKind>>,
    pub network: Option<Vec<NetKind>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    pub url: String,
    pub resize: Option<SizeString>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StorageKind {
    DiskFile(DiskFile),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskFile {
    pub local: PathBuf,
    pub size: SizeString,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NetKind {
    Vlan(Vlan),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vlan {
    pub vlan: u32,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_deser() {
        let yaml = "
          kind: Machine
          name: my-test-vm
          spec:
            cpu: 4
            memory: 8G
            image:
              url: cos://us-south/my-bucket/my-image.qcow2
              resize: 100G
            storage:
            - local: localdisk01.qcow2
              size: 200G
            - local: localdisk02.qcow2
              size: 200G
            network:
            - vlan: 208
            - vlan: 209
        ";

        let r: Resource = serde_yaml::from_str(yaml).unwrap();
        let m = match r {
            Resource::Machine(m) => m,
        };

        assert_eq!(m.name, "my-test-vm");
        assert_eq!(m.spec.cpu, 4);
    }

    #[test]
    fn test_serde() {
        let m = Machine {
            status: None,
            name: "my-test-vm".into(),
            spec: Spec {
                cpu: 4,
                memory: "8G".into(),
                image: Image {
                    url: "cos://us-south/my-bucket/my-image.qcow2".into(),
                    resize: Some("100G".into()),
                },
                storage: Some(vec![
                    StorageKind::DiskFile(DiskFile {
                        local: "localdisk01.qcow2".into(),
                        size: "200G".into(),
                    }),
                    StorageKind::DiskFile(DiskFile {
                        local: "localdisk02.qcow2".into(),
                        size: "200G".into(),
                    }),
                ]),
                network: Some(vec![
                    NetKind::Vlan(Vlan { vlan: 208 }),
                    NetKind::Vlan(Vlan { vlan: 209 }),
                ]),
            },
        };

        let out = serde_yaml::to_string(&m).unwrap();
        println!("{}", out);

        let r: Machine = serde_yaml::from_str(&out).unwrap();
        println!("{:#?}", r);

        assert_eq!(m.name, r.name);
    }

    #[test]
    fn test_sizestring_to_size() {
        assert_eq!(to_size("100M").unwrap(), 100_000_000);
        assert_eq!(to_size("10m").unwrap(), 10_000_000);
        assert_eq!(to_size("20G").unwrap(), 20_000_000_000);
        assert_eq!(to_size("12g").unwrap(), 12_000_000_000);
        assert_eq!(to_size("12Gi").unwrap(), 12 * 1024 * 1024 * 1024);

        assert!(to_size("12Timmies").is_err());
    }
}
