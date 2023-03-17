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
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

use crate::error::Error;
use crate::lockfile::LockFile;

pub struct ImageRepo {
    path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    pub id: String,
    pub path: PathBuf,
    pub origin: String,
    pub format: String,
}

impl ImageRepo {
    pub fn new() -> Self {
        let path = Path::new("/var/lib/bigiron/images");
        if !path.exists() {
            std::fs::create_dir_all(path).expect("error creating image repo directory");
        }

        Self {
            path: path.to_path_buf(),
        }
    }

    fn lockfile(&self) -> LockFile {
        LockFile::new(self.path.join(".lock"))
    }

    pub fn add_from_url(&self, url: Url) -> Result<Image, Error> {
        match url.scheme() {
            "file" => {}
            //"http" | "https" | "file" => {},
            _ => return Err(format!("Url scheme not supported: {:?}", url.scheme()).into()),
        };

        let lf = self.lockfile();
        let _lock = lf.acquire();
        if url.scheme() == "file" {
            let from_path = url
                .to_file_path()
                .expect("error converting URL to filepath");

            let mut h = Sha256::new();
            let mut f = std::fs::File::open(&from_path)?;
            let _ = std::io::copy(&mut f, &mut h)?;
            let r = h.finalize();
            let hx = hex::encode(r);

            let to_path = self.path.join(&hx);
            if !to_path.exists() {
                eprintln!("copying new image from {:?} to {:?}", from_path, to_path);
                std::fs::copy(&from_path, &to_path).expect("error copying image file to repo");
            }

            let img = Image {
                id: hx.clone(),
                path: to_path,
                origin: url.to_string(),
                format: "qcow2".to_string(),
            };

            let imf = self.path.join(format!("{}.json", hx));
            let buf = serde_yaml::to_string(&img).expect("error serializing image json file");
            std::fs::write(&imf, buf.as_bytes()).expect("error writing image json file");

            return Ok(img);
        }

        unimplemented!();
    }

    pub fn get(&self, id: &str) -> Result<Image, Error> {
        let lf = self.lockfile();
        let _lock = lf.acquire();

        let imf = self.path.join(format!("{}.json", id));
        let f = std::fs::File::open(&imf)?;
        let img: Image = serde_yaml::from_reader(&f)?;
        Ok(img)
    }
}
