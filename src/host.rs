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

use sysinfo::{System, SystemExt};


type Error = Box<dyn std::error::Error>;

pub struct HostAgent {
    sys: System
}

pub struct Job {}


impl HostAgent {
    pub fn new() -> Self {
        Self {
            sys: System::new_all(),
        }
    }

    pub fn get_jobs(&self) -> Vec<Job> {
        vec![]
    }

    pub fn get_usage(&self) -> Result<String, Error>{
        Ok("".to_string())
    }

    pub fn report(&mut self) -> String {
        self.sys.refresh_all();
        format!("Capacity: cpu={} memory_kb={}", self.sys.cpus().len(), self.sys.total_memory())
    }
}
