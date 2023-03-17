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

use tracing::info;

// mkdir based mandatory locking for interprocess use
pub struct LockFile {
    path: PathBuf,
}

pub struct LockFileGuard<'a> {
    lf: &'a LockFile,
}

impl Drop for LockFileGuard<'_> {
    fn drop(&mut self) {
        self.lf.release();
    }
}

impl LockFile {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn acquire(&self) -> LockFileGuard {

        loop {
            match std::fs::create_dir(&self.path) {
                Ok(_) => break,
                Err(_) => { info!("Blocked on acquiring lockfile {:?}", self.path) },
            }

            let dur = std::time::Duration::from_millis(1);
            std::thread::sleep(dur);
        }

        // write PID to file inside directory to indicate who has the lock
        let pid = std::process::id();
        let buf = pid.to_string();
        std::fs::write(self.path.join("pid"), buf.as_bytes()).expect("error writing PID to lockfile");

        LockFileGuard{lf: self}
    }

    fn release(&self) {
        // remove pid file 
        std::fs::remove_file(self.path.join("pid")).expect("error while unlinking pid file for lockfile");

        // remove directory (i.e. the lock)
        std::fs::remove_dir(&self.path).expect("error while removing lockfile");
    }
}
