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

use std::path::Path;

use crate::error::Error;
use crate::models;

pub fn define<P: AsRef<Path>>(machine: &models::Machine, image_file: P, bridge_name: &str, macaddr: &str) -> Result<(), Error> {

    let xml = format!(r#"
<domain type='kvm'>
  <name>{name}</name>
  <memory unit="bytes">{memory_bytes}</memory>
  <currentMemory unit="bytes">{memory_bytes}</currentMemory>
  <vcpu>{cpus}</vcpu>
  <os>
    <type arch='x86_64' machine='pc'>hvm</type>
    <boot dev='hd'/>
  </os>
  <features>
    <acpi/>
    <apic/>
  </features>
  <clock offset='utc'/>
  <pm>
    <suspend-to-mem enabled='no'/>
    <suspend-to-disk enabled='no'/>
  </pm>
  <devices>
    <emulator>/usr/bin/kvm</emulator>
    <disk type='file' device='disk'>
      <driver name='qemu' type='qcow2' cache='writeback'/>
      <source file='{image_file}'/>
      <target dev='vda' bus='virtio'/>
    </disk>
    <serial type='pty'>
      <source path='/dev/pts/0'/>
      <target type='isa-serial' port='0'/>
    </serial>
    <input type='keyboard' bus='ps2'/>
    <input type='mouse' bus='ps2'/>
    <interface type="bridge">
      <source bridge="{management_bridge}"/>
      <mac address="{macaddr}"/>
    </interface>
    <memballoon model='virtio'/>
  </devices>
</domain>
    "#,
        name=&machine.name,
        memory_bytes=crate::models::to_size(&machine.spec.memory)?,
        cpus=machine.spec.cpu,
        image_file=image_file.as_ref().to_str().unwrap(),
        management_bridge=bridge_name,
        macaddr=macaddr
    );

    use virt::{connect::Connect, domain::Domain};
    let c = Connect::open("")?;
    let _dom = Domain::create_xml(&c, &xml.to_string(), 0)?;
    Ok(())
}

pub fn destroy(name: &str) -> Result<(), Error> {
    use virt::{connect::Connect, domain::Domain};
    let c = Connect::open("")?;
    let dom = Domain::lookup_by_name(&c, name);
    if let Err(ref e) = dom {
        if e.to_string().contains("Domain not found") {
            return Ok(());
        }
        dom?;
    } else {
        dom.unwrap().destroy()?;
    }
    Ok(())
}
