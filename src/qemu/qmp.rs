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

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Greeting {
    #[serde(rename = "QMP")]
    qmp: GreetingInner,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GreetingInner {
    version: VersionInfo,
    capabilities: Vec<QMPCapability>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VersionInfo {
    qemu: VersionTriple,
    package: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VersionTriple {
    major: u64,
    minor: u64,
    micro: u64,
}

type QMPCapability = String;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Response {
    Return(Return),
    Error(Error),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Return {
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Error {
    class: String,
    desc: String,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl Error {
    pub fn desc(&self) -> &str {
        &self.desc
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Event {
    timestamp: Timestamp,
    event: String,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Timestamp {
    seconds: u64,
    microseconds: u64,
}
