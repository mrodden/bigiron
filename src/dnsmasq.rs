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

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use libc;
use tracing::debug;

pub struct Dnsmasq {
    path: PathBuf,
}

impl Dnsmasq {
    pub fn new() -> Self {
        let path = Path::new("/var/lib/bigiron/dnsmasq");

        let s = Self {
            path: path.to_path_buf(),
        };

        if !s.hostsdir().exists() {
            std::fs::create_dir_all(path).expect("error creating dnsmasq state directories");
        }

        s
    }

    pub fn hostsdir(&self) -> PathBuf {
        self.path.join("hosts")
    }

    pub fn leasefile(&self) -> PathBuf {
        self.path.join("leases")
    }

    pub fn pidfile(&self) -> PathBuf {
        self.path.join("dnsmasq.pid")
    }

    pub fn start(&self) {
        let mut cmd = Command::new("/usr/sbin/dnsmasq");
        let confpath = self.path.join("conf");

        cmd.arg("--strict-order");
        cmd.arg("--bind-interfaces");
        cmd.arg(format!("--pid-file={}", self.pidfile().to_str().unwrap()));
        cmd.arg(format!(
            "--dhcp-hostsdir={}",
            self.hostsdir().to_str().unwrap()
        ));
        //cmd.arg(format!("--dhcp-leasefile={}", self.leasefile().to_str().unwrap()));
        cmd.arg(format!("--conf-file={}", confpath.to_str().unwrap()));
        cmd.arg("--dhcp-range=set:mgmt,172.20.0.2,static,255.255.255.0,30m");
        //cmd.arg("--dhcp-range=set:mgmt,172.20.0.2,172.20.0.254,255.255.255.0,30m");
        cmd.arg("--interface=br0");
        cmd.arg("--except-interface=lo");
        //cmd.arg("--listen-address=172.20.0.1");
        cmd.arg("--domain=cloud.local");
        cmd.arg("--dhcp-authoritative");
        cmd.arg("--dhcp-option=3");
        cmd.arg("--port=0");
        cmd.arg("--dhcp-script=/usr/local/sbin/bigiron-dhcpbridge");
        cmd.arg("--leasefile-ro");

        std::fs::write(&confpath, b"").unwrap();
        std::fs::create_dir_all(&self.hostsdir()).expect("error creating hostsdir");

        debug!("Running: {:?}", cmd);

        let _ = cmd.spawn();
    }

    pub fn stop(&self) {
        self.send_signal(libc::SIGTERM);
    }

    fn send_signal(&self, signal: i32) {
        if self.pidfile().exists() {
            let mut buf = String::new();
            let mut f =
                std::fs::File::open(&self.pidfile()).expect("error opening dnsmasq pidfile");
            f.read_to_string(&mut buf).unwrap();
            let pid = buf.trim().parse::<i32>().unwrap();

            let r = unsafe { libc::kill(pid, signal) };
            if r != 0 {
                panic!("failed to send signal to dnsmasq daemon");
            }
        } else {
            panic!("no pid file found");
        }
    }

    pub fn add_host(&self, mac: &str, ip: &str, hostname: &str) {
        // <macaddr>,<ipaddr>,<hostname>,<leasetime>

        let leasetime = 1 * 60 * 60;

        let buf = format!("{},{},{},{}", mac, ip, hostname, leasetime);
        let fp = self.hostsdir().join(hostname);
        std::fs::write(&fp, &buf).expect("error creating host record file");
    }

    pub fn rm_host(&self, hostname: &str) {
        let fp = self.hostsdir().join(hostname);
        if fp.exists() {
            std::fs::remove_file(&fp).expect("error removing host record file");
            // dnsmasq needs notification to re-read hostsdir when removing files
            self.send_signal(libc::SIGHUP);
        }
    }
}
