use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use anyhow::{Result, anyhow};
use virt::connect::Connect;
use virt::domain::{Domain, DomainState};
use virt::sys;
use xmltree::{Element, XMLNode};

use crate::hostinfo;
use crate::osdata::{self, OsProfile};

pub const OS_MARKER: &str = "VMM-KVM:os=";

#[derive(Clone)]
pub struct Hw {
    pub vcpu: u32,
    pub ram: u32,
    pub kvm: bool,
    pub machine: String,
    pub uefi: bool,
    pub tpm: bool,
    pub video: String,
    pub autostart: bool,
}

#[derive(Clone)]
pub struct DiskInfo {
    pub target: String,
    pub bus: String,
    pub device: String,
    pub file: String,
    pub size_gb: u32,
}

#[derive(Clone)]
pub struct NetInfo {
    pub mac: String,
    pub model: String,
    pub source: String,
    pub iface_type: String,
}

#[derive(Clone)]
pub struct SnapInfo {
    pub name: String,
    pub desc: String,
    pub created: String,
}

#[derive(Clone)]
pub struct Vm {
    pub name: String,
    pub state: u8,
    pub vcpus: u32,
    pub maxmem_mb: u32,
    pub live_vcpus: u32,
    pub live_maxmem_mb: u32,
    pub disk_gb: u32,
    pub os_id: String,
    pub autostart: bool,
}

impl Vm {
    pub fn running(&self) -> bool {
        self.state == 1
    }
    pub fn paused(&self) -> bool {
        self.state == 3
    }
    pub fn os(&self) -> &'static OsProfile {
        osdata::find(&self.os_id)
    }
}

pub struct Backend {
    conn: Connect,
    uri: String,
    cpu_cache: HashMap<String, (Instant, u64)>,
}

fn child<'a>(el: &'a Element, name: &str) -> Option<&'a Element> {
    el.children.iter().find_map(|n| match n {
        XMLNode::Element(e) if e.name == name => Some(e),
        _ => None,
    })
}

fn child_mut<'a>(el: &'a mut Element, name: &str) -> Option<&'a mut Element> {
    el.children.iter_mut().find_map(|n| match n {
        XMLNode::Element(e) if e.name == name => Some(e),
        _ => None,
    })
}

fn remove_children(el: &mut Element, name: &str) {
    el.children
        .retain(|n| !matches!(n, XMLNode::Element(e) if e.name == name));
}

fn el_text(el: &Element) -> String {
    el.children
        .iter()
        .filter_map(|n| match n {
            XMLNode::Text(t) => Some(t.as_str()),
            _ => None,
        })
        .collect()
}

fn set_el_text(el: &mut Element, text: &str) {
    el.children.retain(|n| !matches!(n, XMLNode::Text(_)));
    el.children.push(XMLNode::Text(text.to_string()));
}

fn parse(xml: &str) -> Result<Element> {
    Ok(Element::parse(Cursor::new(xml.as_bytes()))?)
}

fn xml_string(el: &Element) -> Result<String> {
    let mut buf = Vec::new();
    el.write(&mut buf)?;
    Ok(String::from_utf8(buf)?)
}

fn mem_mib(el: Option<&Element>) -> u32 {
    let Some(el) = el else { return 0 };
    let text = el_text(el);
    let Ok(val) = text.trim().parse::<u64>() else {
        return 0;
    };
    match el
        .attributes
        .get("unit")
        .map(|s| s.as_str())
        .unwrap_or("KiB")
    {
        "KiB" | "K" | "k" => (val / 1024) as u32,
        "MiB" | "M" | "m" => val as u32,
        "GiB" | "G" | "g" => (val * 1024) as u32,
        "bytes" => (val / (1024 * 1024)) as u32,
        _ => (val / 1024) as u32,
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn safe_dir(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn qcow2_gb(path: &str) -> Option<u32> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic).ok()?;
    if &magic != b"QFI\xfb" {
        let len = file.metadata().ok()?.len();
        return Some((len.div_ceil(1 << 30)).max(1) as u32);
    }
    file.seek(SeekFrom::Start(24)).ok()?;
    let mut buf = [0u8; 8];
    file.read_exact(&mut buf).ok()?;
    let size = u64::from_be_bytes(buf);
    Some((size.div_ceil(1 << 30)).max(1) as u32)
}

fn disk_gb(root: &Element) -> u32 {
    let Some(devices) = child(root, "devices") else {
        return 0;
    };
    for node in &devices.children {
        let XMLNode::Element(disk) = node else {
            continue;
        };
        if disk.name != "disk" {
            continue;
        }
        if disk
            .attributes
            .get("device")
            .map(|d| d != "disk")
            .unwrap_or(false)
        {
            continue;
        }
        if let Some(src) = child(disk, "source") {
            if let Some(file) = src.attributes.get("file") {
                if let Some(gb) = qcow2_gb(file) {
                    return gb;
                }
            }
        }
    }
    0
}

fn find_ovmf() -> Option<(String, Option<PathBuf>)> {
    const DIRS: &[&str] = &[
        "/usr/share/OVMF",
        "/usr/share/OVMF/x64",
        "/usr/share/OVMF/x64/OVMF",
        "/usr/share/ovmf",
        "/usr/share/ovmf/x64",
        "/usr/share/edk2-ovmf",
        "/usr/share/edk2-ovmf/x64",
        "/usr/share/edk2/ovmf",
        "/usr/share/edk2/x64",
        "/usr/share/qemu",
        "/usr/share/qemu/ovmf",
    ];
    let mut codes = Vec::new();
    let mut vars = Vec::new();
    for dir in DIRS {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if !name.ends_with(".fd") {
                continue;
            }
            let path = entry.path();
            if name.contains("code") {
                codes.push(path);
            } else if name.contains("var") {
                vars.push(path);
            } else if name == "ovmf.fd" {
                codes.push(path);
            }
        }
    }
    let code = codes.into_iter().next()?;
    let code_dir = code.parent().map(|p| p.to_path_buf());
    let var = vars
        .iter()
        .find(|v| v.parent().map(|p| p.to_path_buf()) == code_dir)
        .cloned()
        .or_else(|| vars.into_iter().next());
    Some((code.to_string_lossy().to_string(), var))
}

fn define_domain_with_uefi(conn: &Connect, xml: &str, name: &str, want_uefi: bool) -> Result<()> {
    match conn.define_domain_xml(xml) {
        Ok(_) => return Ok(()),
        Err(first) if want_uefi => {
            let Some((code, vars)) = find_ovmf() else {
                return Err(anyhow!(
                    "UEFI firmware (OVMF) not found: install ovmf/edk2-ovmf ({first})"
                ));
            };
            let mut root = parse(xml)?;
            if let Some(osel) = child_mut(&mut root, "os") {
                osel.attributes.remove("firmware");
                remove_children(osel, "loader");
                remove_children(osel, "nvram");
                remove_children(osel, "firmware");
                let mut loader = Element::new("loader");
                loader.attributes.insert("readonly".into(), "yes".into());
                loader.attributes.insert("type".into(), "pflash".into());
                set_el_text(&mut loader, &code);
                let pos = osel
                    .children
                    .iter()
                    .position(|n| matches!(n, XMLNode::Element(e) if e.name == "type"))
                    .map(|i| i + 1)
                    .unwrap_or(0);
                osel.children.insert(pos, XMLNode::Element(loader));
                if let Some(vars_path) = vars {
                    let dir = home_dir()
                        .join("VirtualMachines")
                        .join("vms")
                        .join(safe_dir(name));
                    std::fs::create_dir_all(&dir)?;
                    let dst = dir.join("nvram_VARS.fd");
                    if !dst.exists() {
                        std::fs::copy(&vars_path, &dst)?;
                    }
                    let mut nv = Element::new("nvram");
                    set_el_text(&mut nv, &dst.to_string_lossy());
                    let pos = osel
                        .children
                        .iter()
                        .position(|n| matches!(n, XMLNode::Element(e) if e.name == "loader"))
                        .map(|i| i + 1)
                        .unwrap_or(0);
                    osel.children.insert(pos, XMLNode::Element(nv));
                }
            }
            conn.define_domain_xml(&xml_string(&root)?)?;
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

fn root_uefi(root: &Element) -> bool {
    let Some(osel) = child(root, "os") else {
        return false;
    };
    osel.attributes
        .get("firmware")
        .map(|value| value == "efi")
        .unwrap_or(false)
        || child(osel, "loader").is_some()
}

fn target_prefix(bus: &str) -> &'static str {
    match bus {
        "ide" => "hd",
        "virtio" => "vd",
        _ => "sd",
    }
}

fn used_targets(root: &Element) -> Vec<String> {
    let Some(devices) = child(root, "devices") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for node in &devices.children {
        let XMLNode::Element(disk) = node else {
            continue;
        };
        if disk.name != "disk" {
            continue;
        }
        if let Some(target) = child(disk, "target") {
            if let Some(dev) = target.attributes.get("dev") {
                out.push(dev.clone());
            }
        }
    }
    out
}

fn free_target(root: &Element, bus: &str) -> String {
    let used = used_targets(root);
    let prefix = target_prefix(bus);
    for letter in 'a'..='z' {
        let candidate = format!("{prefix}{letter}");
        if !used.iter().any(|dev| dev == &candidate) {
            return candidate;
        }
    }
    format!("{prefix}z")
}

fn new_mac() -> String {
    use std::io::Read;
    let mut bytes = [0u8; 3];
    if let Ok(mut file) = std::fs::File::open("/dev/urandom") {
        let _ = file.read_exact(&mut bytes);
    }
    format!(
        "52:54:00:{:02x}:{:02x}:{:02x}",
        bytes[0], bytes[1], bytes[2]
    )
}

fn disk_element(path: &str, target: &str, bus: &str, cdrom: bool, ro: bool) -> Element {
    let mut disk = Element::new("disk");
    disk.attributes.insert("type".into(), "file".into());
    disk.attributes.insert(
        "device".into(),
        if cdrom { "cdrom".into() } else { "disk".into() },
    );
    if !cdrom {
        let mut driver = Element::new("driver");
        driver.attributes.insert("name".into(), "qemu".into());
        driver.attributes.insert("type".into(), "qcow2".into());
        disk.children.push(XMLNode::Element(driver));
    }
    if !path.is_empty() {
        let mut source = Element::new("source");
        source.attributes.insert("file".into(), path.into());
        disk.children.push(XMLNode::Element(source));
    }
    let mut target_el = Element::new("target");
    target_el.attributes.insert("dev".into(), target.into());
    target_el.attributes.insert("bus".into(), bus.into());
    disk.children.push(XMLNode::Element(target_el));
    if cdrom || ro {
        disk.children
            .push(XMLNode::Element(Element::new("readonly")));
    }
    disk
}

fn interface_element(
    iface_type: &str,
    source: Option<(&str, &str)>,
    model: &str,
    mac: &str,
) -> Element {
    let mut iface = Element::new("interface");
    iface.attributes.insert("type".into(), iface_type.into());
    if let Some((key, value)) = source {
        let mut source_el = Element::new("source");
        source_el.attributes.insert(key.into(), value.into());
        iface.children.push(XMLNode::Element(source_el));
    }
    let mut mac_el = Element::new("mac");
    mac_el.attributes.insert("address".into(), mac.into());
    iface.children.push(XMLNode::Element(mac_el));
    let mut model_el = Element::new("model");
    model_el.attributes.insert("type".into(), model.into());
    iface.children.push(XMLNode::Element(model_el));
    iface
}

impl Backend {
    pub fn connect(uri: &str) -> Result<Self> {
        let conn = Connect::open(Some(uri))?;
        Ok(Self {
            conn,
            uri: uri.to_string(),
            cpu_cache: HashMap::new(),
        })
    }

    pub fn is_session(&self) -> bool {
        self.uri.contains("session")
    }

    fn all_domains(&self) -> Result<Vec<Domain>> {
        let flags = sys::VIR_CONNECT_LIST_DOMAINS_ACTIVE | sys::VIR_CONNECT_LIST_DOMAINS_INACTIVE;
        Ok(self.conn.list_all_domains(flags)?)
    }

    fn find(&self, name: &str) -> Result<Domain> {
        self.all_domains()?
            .into_iter()
            .find(|d| d.name().map(|n| n == name).unwrap_or(false))
            .ok_or_else(|| anyhow!("VM not found: {name}"))
    }

    pub fn domains(&self) -> Result<Vec<Vm>> {
        let mut out = Vec::new();
        for dom in self.all_domains()? {
            if let Ok(vm) = self.vm_from_dom(&dom) {
                out.push(vm);
            }
        }
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(out)
    }

    fn vm_from_dom(&self, dom: &Domain) -> Result<Vm> {
        let name = dom.name()?;
        let info = dom.info()?;
        let active = dom.is_active().unwrap_or(false);
        let state = if info.state.is(DomainState::Running) {
            1
        } else if info.state.is(DomainState::Paused) {
            3
        } else {
            5
        };
        let mut vcpus = info.nr_virt_cpu;
        let mut maxmem_mb = (info.max_mem / 1024) as u32;
        let xml = dom.xml_desc(if active {
            sys::VIR_DOMAIN_XML_INACTIVE
        } else {
            0
        })?;
        let mut os_id = osdata::guess(&name).id.to_string();
        let mut disk_size = 0;
        if let Ok(root) = parse(&xml) {
            if let Some(v) = child(&root, "vcpu").map(el_text) {
                if let Ok(n) = v.trim().parse::<u32>() {
                    vcpus = n;
                }
            }
            let m = mem_mib(child(&root, "memory"));
            if m > 0 {
                maxmem_mb = m;
            }
            disk_size = disk_gb(&root);
            if let Some(desc) = child(&root, "description").map(el_text) {
                if let Some(idx) = desc.find(OS_MARKER) {
                    let id = desc[idx + OS_MARKER.len()..]
                        .split('|')
                        .next()
                        .unwrap_or("")
                        .trim();
                    if !id.is_empty() {
                        os_id = id.to_string();
                    }
                }
            }
        }
        Ok(Vm {
            name,
            state,
            vcpus,
            maxmem_mb,
            live_vcpus: info.nr_virt_cpu,
            live_maxmem_mb: (info.max_mem / 1024) as u32,
            disk_gb: disk_size,
            os_id,
            autostart: dom.autostart().unwrap_or(false),
        })
    }

    pub fn start(&self, name: &str) -> Result<()> {
        self.find(name)?.create()?;
        Ok(())
    }

    pub fn shutdown(&self, name: &str) -> Result<()> {
        self.find(name)?.shutdown()?;
        Ok(())
    }

    pub fn force_off(&self, name: &str) -> Result<()> {
        self.find(name)?.destroy()?;
        Ok(())
    }

    pub fn reboot(&self, name: &str) -> Result<()> {
        self.find(name)?.reboot(0)?;
        Ok(())
    }

    pub fn pause(&self, name: &str) -> Result<()> {
        self.find(name)?.suspend()?;
        Ok(())
    }

    pub fn resume(&self, name: &str) -> Result<()> {
        self.find(name)?.resume()?;
        Ok(())
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        let dom = self.find(name)?;
        if let Ok(xml) = dom.xml_desc(sys::VIR_DOMAIN_XML_INACTIVE) {
            let dir = home_dir()
                .join("VirtualMachines")
                .join("vms")
                .join(safe_dir(name));
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(dir.join("domain.xml.bak"), xml);
        }
        if dom.undefine().is_ok() {
            return Ok(());
        }
        if let Ok(xml) = dom.xml_desc(sys::VIR_DOMAIN_XML_INACTIVE) {
            if let Ok(mut root) = parse(&xml) {
                if let Some(osel) = child_mut(&mut root, "os") {
                    osel.attributes.remove("firmware");
                    remove_children(osel, "loader");
                    remove_children(osel, "nvram");
                    remove_children(osel, "firmware");
                }
                let _ = self.conn.define_domain_xml(&xml_string(&root)?);
                let _ = dom.undefine();
            }
        }
        Ok(())
    }

    pub fn stats(&mut self, name: &str) -> Result<(f32, u32, u32)> {
        let dom = self.find(name)?;
        let info = dom.info()?;
        let max_mb = (info.max_mem / 1024) as u32;
        let mem_mb = (info.memory / 1024) as u32;
        if !info.state.is(DomainState::Running) {
            self.cpu_cache.remove(name);
            return Ok((0.0, mem_mb, max_mb));
        }
        let now = Instant::now();
        let mut pct = 0.0;
        if let Some((prev_t, prev_cpu)) = self.cpu_cache.get(name) {
            let wall = now.duration_since(*prev_t).as_secs_f64();
            if wall > 0.01 {
                let delta = info.cpu_time.saturating_sub(*prev_cpu) as f64;
                if delta > 0.0 {
                    pct = (100.0 * delta / (wall * 1e9) / info.nr_virt_cpu.max(1) as f64) as f32;
                }
            }
        }
        self.cpu_cache
            .insert(name.to_string(), (now, info.cpu_time));
        Ok((pct.min(100.0), mem_mb, max_mb))
    }

    pub fn hw_read(&self, name: &str) -> Result<Hw> {
        let dom = self.find(name)?;
        let xml = dom.xml_desc(sys::VIR_DOMAIN_XML_INACTIVE)?;
        let root = parse(&xml)?;
        let vcpu = child(&root, "vcpu")
            .map(el_text)
            .and_then(|t| t.trim().parse::<u32>().ok())
            .unwrap_or(1);
        let ram = mem_mib(child(&root, "memory"));
        let kvm = root
            .attributes
            .get("type")
            .map(|s| s == "kvm")
            .unwrap_or(true);
        let os_el = child(&root, "os");
        let machine = os_el
            .and_then(|o| child(o, "type"))
            .and_then(|t| t.attributes.get("machine").cloned())
            .unwrap_or_else(|| "pc".into());
        let uefi = os_el
            .map(|o| {
                o.attributes
                    .get("firmware")
                    .map(|f| f == "efi")
                    .unwrap_or(false)
                    || child(o, "loader").is_some()
            })
            .unwrap_or(false);
        let devices = child(&root, "devices");
        let tpm = devices.map(|d| child(d, "tpm").is_some()).unwrap_or(false);
        let video = devices
            .and_then(|d| child(d, "video"))
            .and_then(|v| child(v, "model"))
            .and_then(|m| m.attributes.get("type").cloned())
            .unwrap_or_else(|| "vga".into());
        Ok(Hw {
            vcpu,
            ram,
            kvm,
            machine,
            uefi,
            tpm,
            video,
            autostart: dom.autostart().unwrap_or(false),
        })
    }

    pub fn hw_apply(&self, name: &str, hw: &Hw) -> Result<bool> {
        let dom = self.find(name)?;
        let xml = dom.xml_desc(sys::VIR_DOMAIN_XML_INACTIVE)?;
        let mut root = parse(&xml)?;

        if let Some(v) = child_mut(&mut root, "vcpu") {
            set_el_text(v, &hw.vcpu.to_string());
        }
        for tag in ["memory", "currentMemory"] {
            if let Some(m) = child_mut(&mut root, tag) {
                m.attributes.insert("unit".into(), "MiB".into());
                set_el_text(m, &hw.ram.to_string());
            }
        }

        root.attributes.insert(
            "type".into(),
            if hw.kvm { "kvm".into() } else { "qemu".into() },
        );
        remove_children(&mut root, "cpu");
        if hw.kvm {
            let mut cpu = Element::new("cpu");
            cpu.attributes.insert("mode".into(), "host-model".into());
            cpu.attributes.insert("check".into(), "partial".into());
            root.children.push(XMLNode::Element(cpu));
        }

        let old_machine = child(&root, "os")
            .and_then(|o| child(o, "type"))
            .and_then(|t| t.attributes.get("machine").cloned())
            .unwrap_or_default();
        if let Some(osel) = child_mut(&mut root, "os") {
            if let Some(t) = child_mut(osel, "type") {
                t.attributes.insert("machine".into(), hw.machine.clone());
            }
            osel.attributes.remove("firmware");
            remove_children(osel, "loader");
            remove_children(osel, "nvram");
            remove_children(osel, "firmware");
            if hw.uefi {
                osel.attributes.insert("firmware".into(), "efi".into());
            }
        }
        if old_machine != hw.machine {
            if let Some(devs) = child_mut(&mut root, "devices") {
                devs.children.retain(|n| {
                    !matches!(n, XMLNode::Element(e) if e.name == "controller" && e.attributes.get("type").map(|t| t == "pci").unwrap_or(false))
                });
            }
        }

        if let Some(feats) = child_mut(&mut root, "features") {
            remove_children(feats, "smm");
            if hw.uefi && hw.tpm {
                let mut smm = Element::new("smm");
                smm.attributes.insert("state".into(), "on".into());
                feats.children.push(XMLNode::Element(smm));
            }
        }

        if let Some(devs) = child_mut(&mut root, "devices") {
            remove_children(devs, "tpm");
            if hw.tpm {
                let mut tpm = Element::new("tpm");
                tpm.attributes.insert(
                    "model".into(),
                    if hw.machine.contains("q35") {
                        "tpm-crb".into()
                    } else {
                        "tpm-tis".into()
                    },
                );
                let mut backend = Element::new("backend");
                backend.attributes.insert("type".into(), "emulator".into());
                backend.attributes.insert("version".into(), "2.0".into());
                backend
                    .attributes
                    .insert("persistent_state".into(), "yes".into());
                tpm.children.push(XMLNode::Element(backend));
                devs.children.push(XMLNode::Element(tpm));
            }
            if let Some(video) = child_mut(devs, "video") {
                if let Some(model) = child_mut(video, "model") {
                    model.attributes.insert("type".into(), hw.video.clone());
                    model.attributes.insert("heads".into(), "1".into());
                    if hw.video == "qxl" {
                        model.attributes.insert("ram".into(), "65536".into());
                        model.attributes.insert("vram".into(), "32768".into());
                    } else {
                        model.attributes.insert("vram".into(), "16384".into());
                    }
                }
            }
        }

        define_domain_with_uefi(&self.conn, &xml_string(&root)?, name, hw.uefi)?;
        let mut deferred = false;
        if dom.is_active().unwrap_or(false) {
            if dom.set_memory(hw.ram as u64 * 1024).is_err() {
                deferred = true;
            }
            if dom.set_vcpus(hw.vcpu).is_err() {
                deferred = true;
            }
        }
        let _ = dom.set_autostart(hw.autostart);
        Ok(deferred)
    }

    pub fn create_vm(
        &self,
        name: &str,
        profile: &OsProfile,
        vcpu: u32,
        ram_mb: u32,
        disk_gb: u32,
        iso: &str,
        kvm: bool,
    ) -> Result<()> {
        if self.find(name).is_ok() {
            return Err(anyhow!("name already taken: {name}"));
        }
        let base = home_dir().join("VirtualMachines");
        let vm_dir = base.join("vms").join(safe_dir(name));
        std::fs::create_dir_all(&vm_dir)?;
        let disk = vm_dir.join("disk.qcow2");
        if disk.exists() {
            return Err(anyhow!("disk already exists: {}", disk.display()));
        }
        let status = Command::new("qemu-img")
            .args(["create", "-f", "qcow2"])
            .arg(&disk)
            .arg(format!("{disk_gb}G"))
            .status()?;
        if !status.success() {
            return Err(anyhow!("qemu-img failed"));
        }

        let machine = profile.machine;
        let disk_dev = match profile.bus {
            "virtio" => "vda",
            "ide" => "hda",
            _ => "sda",
        };
        let cd_dev = if profile.bus == "ide" { "hdc" } else { "sdb" };
        let cd_bus = if profile.bus == "virtio" {
            "sata"
        } else {
            profile.bus
        };
        let mut xml = String::new();
        xml.push_str(&format!(
            "<domain type='{}'><name>{}</name><description>{}{}</description>\
             <memory unit='MiB'>{}</memory><currentMemory unit='MiB'>{}</currentMemory>\
             <vcpu placement='static'>{}</vcpu>",
            if kvm { "kvm" } else { "qemu" },
            esc(name),
            OS_MARKER,
            esc(profile.id),
            ram_mb,
            ram_mb,
            vcpu
        ));
        xml.push_str(&format!(
            "<os{0}><type arch='x86_64' machine='{1}'>hvm</type><boot dev='cdrom'/><boot dev='hd'/></os>",
            if profile.uefi { " firmware='efi'" } else { "" },
            machine
        ));
        xml.push_str("<features><acpi/><apic/>");
        if profile.uefi && profile.tpm {
            xml.push_str("<smm state='on'/>");
        }
        xml.push_str("</features>");
        if kvm {
            xml.push_str("<cpu mode='host-model' check='partial'/>");
        }
        xml.push_str("<devices>");
        xml.push_str(&format!(
            "<disk type='file' device='disk'><driver name='qemu' type='qcow2'/>\
             <source file='{}'/><target dev='{}' bus='{}'/></disk>",
            esc(&disk.to_string_lossy()),
            disk_dev,
            profile.bus
        ));
        if !iso.is_empty() {
            xml.push_str(&format!(
                "<disk type='file' device='cdrom'><source file='{}'/>\
                 <target dev='{}' bus='{}'/><readonly/></disk>",
                esc(iso),
                cd_dev,
                cd_bus
            ));
        }
        if self.is_session() {
            xml.push_str(&format!(
                "<interface type='user'><model type='{}'/></interface>",
                profile.net
            ));
        } else {
            xml.push_str(&format!(
                "<interface type='network'><source network='default'/>\
                 <model type='{}'/></interface>",
                profile.net
            ));
        }
        xml.push_str("<console type='pty'><target type='serial' port='0'/></console>");
        xml.push_str(
            "<channel type='unix'><target type='virtio' name='org.qemu.guest_agent.0'/></channel>",
        );
        xml.push_str("<input type='tablet' bus='usb'/>");
        xml.push_str(
            "<graphics type='spice' autoport='yes' listen='127.0.0.1'>\
                      <listen type='address' address='127.0.0.1'/></graphics>",
        );
        xml.push_str("<sound model='ich6'/>");
        xml.push_str(&format!(
            "<video><model type='{}' heads='1'{}/></video>",
            profile.video,
            if profile.video == "qxl" {
                " ram='65536' vram='32768'"
            } else {
                " vram='16384'"
            }
        ));
        if profile.tpm {
            xml.push_str(&format!(
                "<tpm model='{}'><backend type='emulator' version='2.0' persistent_state='yes'/></tpm>",
                if machine.contains("q35") { "tpm-crb" } else { "tpm-tis" }
            ));
        }
        xml.push_str("<memballoon model='virtio'/></devices></domain>");

        define_domain_with_uefi(&self.conn, &xml, name, profile.uefi)?;
        Ok(())
    }

    pub fn snapshots(&self, name: &str) -> Result<Vec<SnapInfo>> {
        let dom = self.find(name)?;
        let mut out = Vec::new();
        for snap in dom.list_all_snapshots(0)? {
            let Ok(snap_name) = snap.name() else { continue };
            let mut desc = String::new();
            let mut created = String::new();
            if let Ok(xml) = snap.xml_desc(0) {
                if let Ok(root) = parse(&xml) {
                    desc = child(&root, "description").map(el_text).unwrap_or_default();
                    created = child(&root, "creationTime")
                        .map(el_text)
                        .unwrap_or_default();
                }
            }
            out.push(SnapInfo {
                name: snap_name,
                desc,
                created,
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    pub fn snap_create(&self, name: &str, snapname: &str, desc: &str) -> Result<()> {
        let dom = self.find(name)?;
        let xml = format!(
            "<domainsnapshot><name>{}</name><description>{}</description></domainsnapshot>",
            esc(snapname),
            esc(desc)
        );
        dom.create_snapshot_xml(&xml, 0)?;
        Ok(())
    }

    pub fn snap_revert(&self, name: &str, snapname: &str) -> Result<()> {
        let dom = self.find(name)?;
        Domain::lookup_snapshot_by_name(&dom, snapname, 0)?.revert(0)?;
        Ok(())
    }

    pub fn snap_delete(&self, name: &str, snapname: &str) -> Result<()> {
        let dom = self.find(name)?;
        Domain::lookup_snapshot_by_name(&dom, snapname, 0)?.delete(0)?;
        Ok(())
    }

    pub fn disks(&self, name: &str) -> Result<Vec<DiskInfo>> {
        let dom = self.find(name)?;
        let xml = dom.xml_desc(sys::VIR_DOMAIN_XML_INACTIVE)?;
        let root = parse(&xml)?;
        let Some(devices) = child(&root, "devices") else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for node in &devices.children {
            let XMLNode::Element(disk) = node else {
                continue;
            };
            if disk.name != "disk" {
                continue;
            }
            let target = child(disk, "target");
            let source = child(disk, "source");
            let file = source
                .and_then(|el| el.attributes.get("file").cloned())
                .unwrap_or_default();
            let device = disk
                .attributes
                .get("device")
                .cloned()
                .unwrap_or_else(|| "disk".into());
            let size_gb = if device == "cdrom" {
                0
            } else {
                qcow2_gb(&file).unwrap_or(0)
            };
            out.push(DiskInfo {
                target: target
                    .and_then(|el| el.attributes.get("dev").cloned())
                    .unwrap_or_default(),
                bus: target
                    .and_then(|el| el.attributes.get("bus").cloned())
                    .unwrap_or_default(),
                device,
                file,
                size_gb,
            });
        }
        Ok(out)
    }

    pub fn add_disk(&self, name: &str, size_gb: u32, bus: &str) -> Result<String> {
        let dom = self.find(name)?;
        let xml = dom.xml_desc(sys::VIR_DOMAIN_XML_INACTIVE)?;
        let mut root = parse(&xml)?;
        let dir = home_dir()
            .join("VirtualMachines")
            .join("vms")
            .join(safe_dir(name));
        std::fs::create_dir_all(&dir)?;
        let mut index = 1;
        let path = loop {
            let candidate = dir.join(format!("disk{index}.qcow2"));
            if !candidate.exists() {
                break candidate;
            }
            index += 1;
        };
        let status = Command::new("qemu-img")
            .args(["create", "-f", "qcow2"])
            .arg(&path)
            .arg(format!("{size_gb}G"))
            .status()?;
        if !status.success() {
            return Err(anyhow!("qemu-img failed"));
        }
        let target = free_target(&root, bus);
        let disk = disk_element(&path.to_string_lossy(), &target, bus, false, false);
        let disk_xml = xml_string(&disk)?;
        if let Some(devices) = child_mut(&mut root, "devices") {
            devices.children.push(XMLNode::Element(disk));
        }
        define_domain_with_uefi(&self.conn, &xml_string(&root)?, name, root_uefi(&root))?;
        if dom.is_active().unwrap_or(false) {
            let _ = dom.attach_device_flags(&disk_xml, sys::VIR_DOMAIN_AFFECT_LIVE);
        }
        Ok(target)
    }

    pub fn remove_disk(&self, name: &str, target: &str) -> Result<()> {
        let dom = self.find(name)?;
        let xml = dom.xml_desc(sys::VIR_DOMAIN_XML_INACTIVE)?;
        let mut root = parse(&xml)?;
        let mut removed = None;
        if let Some(devices) = child_mut(&mut root, "devices") {
            let mut index = None;
            for (i, node) in devices.children.iter().enumerate() {
                let XMLNode::Element(disk) = node else {
                    continue;
                };
                if disk.name != "disk" {
                    continue;
                }
                let matches = child(disk, "target")
                    .and_then(|el| el.attributes.get("dev"))
                    .map(|dev| dev == target)
                    .unwrap_or(false);
                if matches {
                    removed = Some(disk.clone());
                    index = Some(i);
                    break;
                }
            }
            if let Some(index) = index {
                devices.children.remove(index);
            }
        }
        let Some(removed) = removed else {
            return Err(anyhow!("disk not found: {target}"));
        };
        let removed_xml = xml_string(&removed)?;
        define_domain_with_uefi(&self.conn, &xml_string(&root)?, name, root_uefi(&root))?;
        if dom.is_active().unwrap_or(false) {
            let _ = dom.detach_device_flags(&removed_xml, sys::VIR_DOMAIN_AFFECT_LIVE);
        }
        Ok(())
    }

    pub fn attach_iso(&self, name: &str, iso: Option<&str>) -> Result<()> {
        let dom = self.find(name)?;
        let xml = dom.xml_desc(sys::VIR_DOMAIN_XML_INACTIVE)?;
        let mut root = parse(&xml)?;
        let machine = child(&root, "os")
            .and_then(|osel| child(osel, "type"))
            .and_then(|el| el.attributes.get("machine").cloned())
            .unwrap_or_else(|| "pc".into());
        let bus = if machine.contains("q35") {
            "sata"
        } else {
            "ide"
        };
        let target = free_target(&root, bus);
        let mut existing = false;
        let mut changed = None;
        if let Some(devices) = child_mut(&mut root, "devices") {
            let mut cdrom_index = None;
            for (i, node) in devices.children.iter().enumerate() {
                let XMLNode::Element(disk) = node else {
                    continue;
                };
                if disk.name == "disk"
                    && disk
                        .attributes
                        .get("device")
                        .map(|d| d == "cdrom")
                        .unwrap_or(false)
                {
                    cdrom_index = Some(i);
                    break;
                }
            }
            if let Some(i) = cdrom_index {
                existing = true;
                let XMLNode::Element(disk) = &mut devices.children[i] else {
                    unreachable!()
                };
                let mut source_index = None;
                for (j, node) in disk.children.iter().enumerate() {
                    if matches!(node, XMLNode::Element(e) if e.name == "source") {
                        source_index = Some(j);
                        break;
                    }
                }
                match iso {
                    Some(path) => {
                        if let Some(j) = source_index {
                            if let XMLNode::Element(source) = &mut disk.children[j] {
                                source.attributes.insert("file".into(), path.into());
                            }
                        } else {
                            let mut source = Element::new("source");
                            source.attributes.insert("file".into(), path.into());
                            disk.children.insert(0, XMLNode::Element(source));
                        }
                    }
                    None => {
                        if let Some(j) = source_index {
                            disk.children.remove(j);
                        }
                    }
                }
                changed = Some(disk.clone());
            } else {
                let disk = disk_element(iso.unwrap_or(""), &target, bus, true, true);
                changed = Some(disk.clone());
                devices.children.push(XMLNode::Element(disk));
            }
        }
        define_domain_with_uefi(&self.conn, &xml_string(&root)?, name, root_uefi(&root))?;
        if dom.is_active().unwrap_or(false) {
            if let Some(disk) = changed {
                let disk_xml = xml_string(&disk)?;
                if existing {
                    let _ = dom.update_device_flags(&disk_xml, sys::VIR_DOMAIN_AFFECT_LIVE);
                } else {
                    let _ = dom.attach_device_flags(&disk_xml, sys::VIR_DOMAIN_AFFECT_LIVE);
                }
            }
        }
        Ok(())
    }

    pub fn nics(&self, name: &str) -> Result<Vec<NetInfo>> {
        let dom = self.find(name)?;
        let xml = dom.xml_desc(sys::VIR_DOMAIN_XML_INACTIVE)?;
        let root = parse(&xml)?;
        let Some(devices) = child(&root, "devices") else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for node in &devices.children {
            let XMLNode::Element(iface) = node else {
                continue;
            };
            if iface.name != "interface" {
                continue;
            }
            let source = child(iface, "source");
            let source_value = source
                .and_then(|el| {
                    el.attributes
                        .get("network")
                        .or_else(|| el.attributes.get("bridge"))
                        .or_else(|| el.attributes.get("dev"))
                        .cloned()
                })
                .unwrap_or_default();
            out.push(NetInfo {
                mac: child(iface, "mac")
                    .and_then(|el| el.attributes.get("address").cloned())
                    .unwrap_or_default(),
                model: child(iface, "model")
                    .and_then(|el| el.attributes.get("type").cloned())
                    .unwrap_or_default(),
                source: source_value,
                iface_type: iface
                    .attributes
                    .get("type")
                    .cloned()
                    .unwrap_or_else(|| "user".into()),
            });
        }
        Ok(out)
    }

    pub fn add_nic(&self, name: &str, model: &str) -> Result<String> {
        let dom = self.find(name)?;
        let xml = dom.xml_desc(sys::VIR_DOMAIN_XML_INACTIVE)?;
        let mut root = parse(&xml)?;
        let existing = self.nics(name)?;
        let mut mac = new_mac();
        let mut guard = 0;
        while existing.iter().any(|nic| nic.mac == mac) && guard < 32 {
            mac = new_mac();
            guard += 1;
        }
        let iface = if self.is_session() {
            interface_element("user", None, model, &mac)
        } else {
            interface_element("network", Some(("network", "default")), model, &mac)
        };
        let iface_xml = xml_string(&iface)?;
        if let Some(devices) = child_mut(&mut root, "devices") {
            devices.children.push(XMLNode::Element(iface));
        }
        define_domain_with_uefi(&self.conn, &xml_string(&root)?, name, root_uefi(&root))?;
        if dom.is_active().unwrap_or(false) {
            let _ = dom.attach_device_flags(&iface_xml, sys::VIR_DOMAIN_AFFECT_LIVE);
        }
        Ok(mac)
    }

    pub fn remove_nic(&self, name: &str, mac: &str) -> Result<()> {
        let dom = self.find(name)?;
        let xml = dom.xml_desc(sys::VIR_DOMAIN_XML_INACTIVE)?;
        let mut root = parse(&xml)?;
        let mut removed = None;
        if let Some(devices) = child_mut(&mut root, "devices") {
            let mut index = None;
            for (i, node) in devices.children.iter().enumerate() {
                let XMLNode::Element(iface) = node else {
                    continue;
                };
                if iface.name != "interface" {
                    continue;
                }
                let matches = child(iface, "mac")
                    .and_then(|el| el.attributes.get("address"))
                    .map(|value| value == mac)
                    .unwrap_or(false);
                if matches {
                    removed = Some(iface.clone());
                    index = Some(i);
                    break;
                }
            }
            if let Some(index) = index {
                devices.children.remove(index);
            }
        }
        let Some(removed) = removed else {
            return Err(anyhow!("interface not found: {mac}"));
        };
        let removed_xml = xml_string(&removed)?;
        define_domain_with_uefi(&self.conn, &xml_string(&root)?, name, root_uefi(&root))?;
        if dom.is_active().unwrap_or(false) {
            let _ = dom.detach_device_flags(&removed_xml, sys::VIR_DOMAIN_AFFECT_LIVE);
        }
        Ok(())
    }

    pub fn console(&self, name: &str) -> Result<()> {
        let exe = which("virt-viewer")
            .or_else(|| which("remote-viewer"))
            .ok_or_else(|| {
                anyhow!("virt-viewer not installed (install the virt-viewer package)")
            })?;
        Command::new(exe)
            .arg("-c")
            .arg(&self.uri)
            .arg(name)
            .spawn()?;
        Ok(())
    }
}

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

pub fn iso_files() -> Vec<PathBuf> {
    let dir = home_dir().join("VirtualMachines").join("iso");
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension()
                .map(|x| x.eq_ignore_ascii_case("iso"))
                .unwrap_or(false)
            {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

pub fn host_limits() -> (u32, u32) {
    hostinfo::host_limits()
}

fn which(bin: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|p| p.join(bin))
        .find(|p| p.is_file())
}
