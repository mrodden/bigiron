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

use std::net::Ipv4Addr;
use std::path::Path;
use std::fs::File;

use tracing::warn;
use hex;
use rand::{thread_rng, Rng};
use ipnet::Ipv4Net;
use serde_yaml;
use serde::{Serialize, Deserialize};

use crate::error::Error;
use crate::lockfile::{LockFile, LockFileGuard};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetInfo {
    pub mac: String,
    pub ip: String,
    pub hostname: String,
    allocated: bool,
    leased: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetState {
    cidr: String,
    reservations: Vec<NetInfo>,
}

impl NetState {
    fn new() -> Self {
        Self {
            cidr: "172.20.0.0/24".to_string(),
            reservations: Vec::new(),
        }
    }

    fn from_file<P: AsRef<Path>>(path: P) -> Self {
        let f = File::open(&path).expect("error opening netstate file");
        serde_yaml::from_reader(&f).unwrap()
    }

    fn save<P: AsRef<Path>>(&self, path: P) {
        let buf = serde_yaml::to_string(&self).expect("error while serializing netstate");
        std::fs::write(path.as_ref(), &buf).expect("error while writing netstate file");
    }
}

pub fn generate_mac() -> String {
    let mut rng = thread_rng();

    let mac: [u8; 6] = [
        0x00, 0x16, 0x3e,
        rng.gen_range(0x00..0x7f),
        rng.gen_range(0x00..0xff),
        rng.gen_range(0x00..0xff),
    ];

    let mac_string = mac.map(|v| hex::encode([v])).join(":");
    mac_string
}

pub fn new_reservation(hostname: &str) -> NetInfo {
    let np = Path::new("/var/lib/bigiron/netstate");

    // acquire lockfile
    let lf = LockFile::new("/var/lib/bigiron/netstate.lock");
    let _lock = lf.acquire();
 
    // read any current state or create new
    let mut netstate = match np.exists() {
        true => { NetState::from_file(&np) },
        false => {         NetState::new()        },
    };

    // return a reservations for this hostname if it already exists
    if let Some(netinfo) = netstate.reservations.iter_mut().find(|x| x.hostname == hostname) {
        netinfo.allocated = true;
        let res = netinfo.clone();
        netstate.save(&np);
        return res;
    }

    // loop through IPs in CIDR mask, check if free
    let mut free: Option<Ipv4Addr> = None;
    let net: Ipv4Net = netstate.cidr.parse().unwrap();
    for addr in net.hosts() {
        if addr.to_string().ends_with(".1") || addr.to_string().ends_with(".255") {
            // skip gateway and broadcast ranges
            continue;
        }

        let mut inuse = false;
        for r in &netstate.reservations {
            let ip = r.ip.parse::<Ipv4Addr>().unwrap();
            if ip == addr {
                inuse = true;
                break;
            }
        }

        if !inuse {
            free = Some(addr);
            break;
        }
    }

    if free.is_none() {
        panic!("No more free addresses on network");
    }

    // generate mac and check
    let mut unique = false;
    let mut mac = String::new();
    while !unique {
        mac = generate_mac();
        unique = true;
        for r in &netstate.reservations {
            if mac == r.mac {
                unique = false;
                break;
            }
        }
    }

    // insert reservation, write to disk
    let new_res = NetInfo{
        mac: mac,
        ip: free.unwrap().to_string(),
        hostname: hostname.to_string(),
        allocated: true,
        leased: false,
    };
    netstate.reservations.push(new_res.clone());
    netstate.save(&np);

    // return net info
    new_res
}

fn get_netstate_locked<'a, P: AsRef<Path>>(path: P, lf: &'a LockFile) -> (NetState, LockFileGuard<'a>) {
    if !path.as_ref().exists() {
        panic!("no netstate file found");
    }

    let lock = lf.acquire();

    (NetState::from_file(path.as_ref()), lock)
}

pub fn remove_reservation(hostname: &str) -> Result<(), Error> {
    let np = Path::new("/var/lib/bigiron/netstate");
    let lf = LockFile::new("/var/lib/bigiron/netstate.lock");
    let (mut netstate, _lock) = get_netstate_locked(&np, &lf);

    let mut entry = None;
    for (i, r) in netstate.reservations.iter_mut().enumerate() {
        if r.hostname == hostname {
            r.allocated = false;
            entry = Some((i, r));
            break;
        }
    }

    if let Some((i, r)) = entry {
        if !r.allocated && !r.leased {
            let _ = netstate.reservations.remove(i);
        }
        netstate.save(&np);
    } 
    else {
        warn!("no reservation for {} found to remove", hostname);
    }

    Ok(())
}


pub fn add_lease(mac: &str, addr: &str, hostname: Option<String>) {
    let np = Path::new("/var/lib/bigiron/netstate");
    let lf = LockFile::new("/var/lib/bigiron/netstate.lock");
    let (mut netstate, _lock) = get_netstate_locked(&np, &lf);

    // need to mark the IP address as leased
    if let Some(netinfo) = netstate.reservations.iter_mut().find(|x| x.ip == addr) {
        netinfo.leased = true;
        if netinfo.mac != mac {
            warn!("new lease mac='{}' didn't match reservation='{:?}'", mac, netinfo);
        }
        if hostname.is_some() && netinfo.hostname != hostname.as_ref().unwrap().as_str() {
            warn!("new lease hostname='{}' didn't match reservation='{:?}'", hostname.unwrap(), netinfo);
        }
    } else {
        // create a reservation so we don't try to use the IP
        
        let host = match hostname {
            Some(name) => name.to_string(),
            None => String::new(),
        };

        let new_res = NetInfo{
            mac: mac.to_string(),
            ip: addr.to_string(),
            hostname: host,
            allocated: false,
            leased: true,
        };
        netstate.reservations.push(new_res);
    }

    netstate.save(&np);
}

pub fn del_lease(_mac: &str, addr: &str, _hostname: Option<String>) {
    let np = Path::new("/var/lib/bigiron/netstate");
    let lf = LockFile::new("/var/lib/bigiron/netstate.lock");
    let (mut netstate, _lock) = get_netstate_locked(&np, &lf);

    let mut entry = None;
    for (i, r) in netstate.reservations.iter_mut().enumerate() {
        if r.ip == addr {
            r.leased = false;
            entry = Some((i, r));
            break;
        }
    }

    if let Some((i, r)) = entry {
        if !r.allocated && !r.leased {
            let _ = netstate.reservations.remove(i);
        }

        netstate.save(&np);
    } 
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_generate_mac() {
        let mac = generate_mac();
        eprintln!("{}", mac);
        assert!(mac.starts_with("00:16:3e"));
    }
}
