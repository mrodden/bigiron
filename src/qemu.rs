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
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use std::os::unix::io::AsRawFd;

use fork::Fork;
use serde_json::{json, Value};
use tracing::{debug, info, trace};

mod qmp;

use crate::error::Error;

pub struct Image {
    pub path: PathBuf,
}

pub struct Process {
    base_dir: PathBuf,
    name: String,
    cpus: u32,
    memory_mb: u64,
    uuid: String,
    image: Image,
}

impl Process {
    pub fn new<P: AsRef<Path>>(
        dir: P,
        name: &str,
        cpus: u32,
        memory_mb: u64,
        uuid: &str,
        image: Image,
    ) -> Self {
        let base_dir = dir.as_ref().to_path_buf();

        Self {
            base_dir,
            name: name.to_string(),
            cpus,
            memory_mb,
            uuid: uuid.into(),
            image,
        }
    }

    fn build_cmd(&self, net_fd: i32) -> Command {
        let emulator = "/usr/bin/kvm";
        let mut cmd = Command::new(emulator);

        let args: Vec<&str> = "-machine pc-i440fx-3.1,accel=kvm,usb=off,dump-guest-core=off \
            -realtime mlock=off \
            -display none \
            -no-user-config \
            -nodefaults \
            -rtc base=utc \
            -no-shutdown \
            -global PIIX4_PM.disable_s3=1 \
            -global PIIX4_PM.disable_s4=1 \
            -boot strict=on \
            -device piix3-usb-uhci,id=usb,bus=pci.0,addr=0x1.0x2 \
            -device virtio-blk-pci,scsi=off,bus=pci.0,addr=0x2,drive=drive-virtio-disk0,id=virtio-disk0,bootindex=1,write-cache=on \
            -chardev pty,id=charserial0 \
            -device isa-serial,chardev=charserial0,id=serial0 \
            -device virtio-balloon-pci,id=balloon0,bus=pci.0,addr=0x3 \
            -sandbox on,obsolete=deny,elevateprivileges=deny,spawn=deny,resourcecontrol=deny \
            -msg timestamp=on".split(" ").collect();

        let socket_path = self.base_dir.join("monitor.sock");
        let monitor_mode = "control";
        let image_format = "qcow2";
        let pause_on_start = false;
        let bridge_name = "br0";

        if pause_on_start {
            cmd.arg("-S");
        }

        cmd.args(args)
            .arg("-name")
            .arg(format!("guest={},debug-threads=on", self.name))
            .arg("-chardev")
            .arg(format!(
                "socket,id=charmonitor,path={},server,nowait",
                socket_path.display()
            ))
            .arg("-mon")
            .arg(format!(
                "chardev=charmonitor,id=monitor,mode={}",
                monitor_mode
            ))
            .arg("-m")
            .arg(format!("{}", self.memory_mb))
            .arg("-smp")
            .arg(format!(
                "{},sockets=1,cores={},threads=1",
                self.cpus, self.cpus
            ))
            .arg("-uuid")
            .arg(self.uuid.clone())
            .arg("-drive")
            .arg(format!(
                "file={},format={},if=none,id=drive-virtio-disk0,cache=writeback",
                self.image.path.display(),
                image_format
            ))
            .arg("-device")
            .arg("virtio-net-pci,netdev=net1,mac=52:54:00:b8:9c:58")
            .arg("-netdev")
            .arg(format!("tap,fd={},id=net1", net_fd))
            .arg("-device")
            .arg("virtio-net-pci,netdev=net0")
            .arg("-netdev")
            .arg(format!("bridge,br={},id=net0", bridge_name));

        //.arg("virtio-net-pci,netdev=nic,addr=52:54:00:b8:9c:58")
        cmd
    }

    fn run(&self) {
        let log_path = self.base_dir.join("qemu.log");
        let logfile = File::options()
            .append(true)
            .create(true)
            .open(log_path)
            .expect("error opening file");

        use libc::O_RDWR;

        let cpath = std::ffi::CString::new("/dev/tap7").expect("error building cstring");
        let tap_fd = unsafe { libc::open(cpath.as_ptr(), O_RDWR) };

        // duplicate to high int FD, to avoid conflicting with std{in,out,err} if they are closed
        // prior to the call to this function
        let dup_fd = unsafe { libc::dup2(tap_fd, 24) };
        let _ = unsafe { libc::close(tap_fd) };

        let mut cmd = self.build_cmd(dup_fd);

        cmd.stdin(Stdio::null())
            .stderr(logfile.try_clone().unwrap())
            .stdout(logfile);

        let child = cmd.spawn().unwrap();
        let pid = child.id();

        let _ = unsafe { libc::close(dup_fd) };

        // record PID file
        let pid_path = self.base_dir.join("pid");
        let mut pidfile = File::create(pid_path).expect("error opening file");
        write!(&mut pidfile, "{}", pid).unwrap();
    }

    pub fn launch(&self) {
        match fork::fork() {
            Ok(Fork::Child) => {
                fork::setsid().unwrap();
                fork::chdir().unwrap();
                fork::close_fd().unwrap();

                self.run();

                std::process::exit(0);
            }
            Ok(Fork::Parent(_)) => {
                return;
            }
            Err(e) => {
                panic!("{}", e);
            }
        }
    }
}

pub struct Monitor {
    stream: UnixStream,
}

impl Monitor {
    pub fn connect<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let mut s = UnixStream::connect(path)?;

        let mut buf = [0u8; 4096];
        let n = s.read(&mut buf)?;
        let _greeting: qmp::Greeting = serde_json::from_slice(&mut buf[..n])?;

        let caps = json!({
            "execute": "qmp_capabilities",
            "arguments": {},
        });
        write!(&mut s, "{}", caps)?;

        let resp = read_response(&mut s)?;
        if let qmp::Response::Error(err) = resp {
            return Err(format!("Error from qemu monitor: {:?}", err.desc()).into());
        }

        Ok(Self { stream: s })
    }

    fn execute(&mut self, command: &str) -> Result<qmp::Return, Error> {
        let caps = json!({
            "execute": command,
        });
        write!(&mut self.stream, "{}", caps)?;

        let resp = read_response(&mut self.stream)?;
        match resp {
            qmp::Response::Error(err) => {
                return Err(format!("Error from qemu monitor: {:?}", err.desc()).into());
            }
            qmp::Response::Return(ret) => {
                debug!("{:?}", ret);
                Ok(ret)
            }
        }
    }

    pub fn quit(&mut self) -> Result<(), Error> {
        self.execute("quit")?;
        Ok(())
    }

    pub fn status(&mut self) -> Result<String, Error> {
        let ret = self.execute("query-status")?;
        let status = ret
            .extra
            .get("status")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();
        Ok(status)
    }

    pub fn cont(&mut self) -> Result<(), Error> {
        self.execute("cont")?;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), Error> {
        self.execute("stop")?;
        Ok(())
    }
}

fn read_response(s: &mut UnixStream) -> Result<qmp::Response, Error> {
    let mut buf = [0u8; 4096];

    while select::has_data(s.as_raw_fd(), true) {
        let n = s.read(&mut buf)?;
        trace!(
            "From monitor: {:?}",
            std::str::from_utf8(&buf[..n]).unwrap()
        );

        for val in parse_qapi_stream(&mut buf[..n]) {
            if val.get("event").is_some() {
                let event: qmp::Event = serde_json::from_value(val)?;
                info!("{:?}", event);
            } else {
                let resp: qmp::Response = serde_json::from_value(val)?;
                return Ok(resp);
            }
        }
    }

    Err("no reponse found".into())
}

fn parse_qapi_stream(buf: &mut [u8]) -> Vec<Value> {
    let mut vals = Vec::new();
    for slice in buf.split_mut(|byt| byt == &b"\r"[0]) {
        if slice.len() == 0 || (slice.len() == 1 && slice[0] == b"\n"[0]) {
            continue;
        }

        let val: Value = serde_json::from_slice(slice).unwrap();
        vals.push(val);
    }
    return vals;
}

mod select {
    use std::mem::MaybeUninit;
    use std::ptr;

    use libc::{fd_set, select, timeval, FD_SET, FD_ZERO};

    fn new_fd_set() -> fd_set {
        let mut set = MaybeUninit::uninit();
        unsafe {
            FD_ZERO(set.as_mut_ptr());
            set.assume_init()
        }
    }

    pub fn has_data(fd: i32, block: bool) -> bool {
        let mut rfds = new_fd_set();

        // a tv of 0 means return immediately, or poll
        let mut tv = timeval {
            tv_sec: 0,
            tv_usec: 0,
        };

        unsafe {
            FD_SET(fd, &mut rfds);
        }

        let res = unsafe {
            select(
                fd + 1,
                &mut rfds,
                ptr::null_mut(),
                ptr::null_mut(),
                if !block { &mut tv } else { ptr::null_mut() },
            )
        };

        if res == -1 {
            eprintln!("Error in select()");
        }

        res == 1
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_qapi_stream() {
        let s = b"{\"timestamp\": {\"seconds\": 1677200460, \"microseconds\": 774479}, \"event\": \"STOP\"}\r\n{\"return\": {}}\r\n";
        let mut input = s.to_vec();
        let vals = parse_qapi_stream(&mut input[..]);
        eprintln!("{:?}", vals);

        assert_eq!(vals.len(), 2);
        assert_eq!(
            vals[0],
            json!({"timestamp": { "seconds": 1677200460, "microseconds": 774479 }, "event": "STOP"})
        );
        assert_eq!(vals[1], json!({"return": {}}));
    }
}
