use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::time::Duration;

use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, PromptLevel, Render, Rgba, SharedString,
    Stateful, Timer, Window, div, prelude::*, px, rgb, svg,
};

use crate::backend::{self, Hw, SnapInfo, Vm};
use crate::hostinfo;
use crate::osdata::{self, OsProfile};

const BG: u32 = 0x0a141f;
const PANEL: u32 = 0x0d1926;
const CARD: u32 = 0x132434;
const CARD2: u32 = 0x16293c;
const BORDER: u32 = 0x1f3a52;
const ACCENT: u32 = 0x2dd4bf;
const ACCENT_D: u32 = 0x17a493;
const TEXT: u32 = 0xdbe7f1;
const MUTED: u32 = 0x7e97ac;
const GREEN: u32 = 0x4ade80;
const AMBER: u32 = 0xfbbf24;
const RED: u32 = 0xf87171;

fn col(hex: u32) -> Rgba {
    rgb(hex)
}

fn alpha(color: Rgba, a: f32) -> Rgba {
    Rgba { a, ..color }
}

fn fmt_mb(mb: u32) -> String {
    if mb >= 1024 {
        format!("{:.1} GiB", mb as f64 / 1024.0)
    } else {
        format!("{mb} MiB")
    }
}

fn fmt_disk(gb: u32) -> String {
    if gb == 0 {
        "—".to_string()
    } else {
        format!("{gb} GiB")
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Vms,
    Create,
    Storage,
    Networks,
    Ssh,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Hardware,
    Snapshots,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StatusKind {
    Info,
    Ok,
    Error,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BtnKind {
    Normal,
    Accent,
    Danger,
}

#[derive(Clone, Copy)]
enum HwField {
    Vcpu,
    Ram,
}

#[derive(Clone, PartialEq, Eq)]
enum SnapArm {
    Revert(String),
    Delete(String),
}

#[derive(Clone, Copy)]
enum VmAction {
    Start,
    Shutdown,
    ForceOff,
    Reboot,
    Pause,
    Resume,
    Console,
}

#[derive(Default)]
struct VmStat {
    cpu: VecDeque<f32>,
    ram: VecDeque<f32>,
}

struct CreateForm {
    isos: Vec<PathBuf>,
    iso: Option<usize>,
    profile_id: String,
    version: String,
    name: String,
    vcpu: u32,
    ram: u32,
    disk: u32,
    kvm: bool,
}

impl CreateForm {
    fn new(help_ram: u32, help_cpus: u32, kvm: bool) -> Self {
        Self {
            isos: backend::iso_files(),
            iso: None,
            profile_id: "other".into(),
            version: String::new(),
            name: "vm".into(),
            vcpu: 2.min(help_cpus.max(1)),
            ram: 2048.min(help_ram.max(256)),
            disk: 20,
            kvm,
        }
    }

    fn defaults(&mut self, profile: &OsProfile, host_ram: u32, host_cpus: u32, kvm_ok: bool) {
        self.vcpu = profile.vcpu.clamp(1, host_cpus.max(1));
        self.ram = profile.ram_mb.clamp(256, host_ram.max(256));
        self.disk = profile.disk_gb.max(5);
        self.kvm = kvm_ok;
    }
}

pub struct VmmApp {
    backend: Option<backend::Backend>,
    conn_uri: String,
    conn_err: Option<String>,
    vms: Vec<Vm>,
    selected: Option<String>,
    screen: Screen,
    tab: Tab,
    hw: Option<Hw>,
    snaps: Vec<SnapInfo>,
    snap_arm: Option<SnapArm>,
    stats: HashMap<String, VmStat>,
    form: CreateForm,
    host_ram_mb: u32,
    host_cpus: u32,
    kvm_ok: bool,
    status: Option<(String, StatusKind)>,
    ticks: u64,
    started: bool,
}

impl VmmApp {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        let (host_ram_mb, host_cpus) = backend::host_limits();
        let kvm_ok = hostinfo::kvm_available();
        let mut backend = None;
        let mut conn_err = None;
        let mut conn_uri = String::new();
        for uri in ["qemu:///session", "qemu:///system"] {
            match backend::Backend::connect(uri) {
                Ok(b) => {
                    conn_uri = uri.to_string();
                    backend = Some(b);
                    conn_err = None;
                    break;
                }
                Err(e) => conn_err = Some(format!("{uri}: {e}")),
            }
        }
        let form = CreateForm::new(host_ram_mb, host_cpus, kvm_ok);
        let mut app = Self {
            backend,
            conn_uri,
            conn_err,
            vms: Vec::new(),
            selected: None,
            screen: Screen::Vms,
            tab: Tab::Overview,
            hw: None,
            snaps: Vec::new(),
            snap_arm: None,
            stats: HashMap::new(),
            form,
            host_ram_mb,
            host_cpus,
            kvm_ok,
            status: None,
            ticks: 0,
            started: false,
        };
        app.form_sync_name();
        app.refresh();
        app
    }

    fn set_status(&mut self, msg: impl Into<String>, kind: StatusKind) {
        self.status = Some((msg.into(), kind));
    }

    fn selected_vm(&self) -> Option<&Vm> {
        self.selected
            .as_ref()
            .and_then(|n| self.vms.iter().find(|v| &v.name == n))
    }

    fn load_details(&mut self) {
        self.hw = self
            .selected
            .as_ref()
            .and_then(|name| self.backend.as_ref().and_then(|b| b.hw_read(name).ok()));
        self.snaps = self
            .selected
            .as_ref()
            .and_then(|name| self.backend.as_ref().and_then(|b| b.snapshots(name).ok()))
            .unwrap_or_default();
    }

    fn reload_snaps(&mut self) {
        self.snaps = self
            .selected
            .as_ref()
            .and_then(|name| self.backend.as_ref().and_then(|b| b.snapshots(name).ok()))
            .unwrap_or_default();
    }

    fn ensure_selection(&mut self) {
        let exists = self
            .selected
            .as_ref()
            .map(|s| self.vms.iter().any(|v| &v.name == s))
            .unwrap_or(false);
        if !exists {
            self.selected = self.vms.first().map(|v| v.name.clone());
            self.load_details();
            self.snap_arm = None;
        }
    }

    fn refresh(&mut self) {
        let Some(b) = self.backend.as_ref() else {
            return;
        };
        match b.domains() {
            Ok(vms) => {
                self.vms = vms;
                self.conn_err = None;
                self.ensure_selection();
            }
            Err(e) => self.conn_err = Some(e.to_string()),
        }
    }

    fn reconnect(&mut self, cx: &mut Context<Self>) {
        self.conn_err = None;
        for uri in ["qemu:///session", "qemu:///system"] {
            match backend::Backend::connect(uri) {
                Ok(b) => {
                    self.conn_uri = uri.to_string();
                    self.backend = Some(b);
                    self.refresh();
                    self.set_status(format!("Connected to {uri}"), StatusKind::Ok);
                    cx.notify();
                    return;
                }
                Err(e) => self.conn_err = Some(format!("{uri}: {e}")),
            }
        }
        cx.notify();
    }

    fn start_tick(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(Duration::from_secs(1)).await;
                let alive = this
                    .update(cx, |this, cx| {
                        this.tick();
                        cx.notify();
                    })
                    .is_ok();
                if !alive {
                    break;
                }
            }
        })
        .detach();
    }

    fn tick(&mut self) {
        self.ticks += 1;
        if self.backend.is_none() {
            return;
        }
        if self.ticks % 2 == 0 {
            self.refresh();
        }
        let running: Vec<String> = self
            .vms
            .iter()
            .filter(|v| v.running())
            .map(|v| v.name.clone())
            .collect();
        let mut updates = Vec::new();
        if let Some(b) = self.backend.as_mut() {
            for name in &running {
                if let Ok((cpu, mem, max)) = b.stats(name) {
                    updates.push((name.clone(), cpu, mem, max));
                }
            }
        }
        for (name, cpu, mem, max) in updates {
            let stat = self.stats.entry(name).or_default();
            stat.cpu.push_back(cpu);
            if stat.cpu.len() > 48 {
                stat.cpu.pop_front();
            }
            let pct = if max > 0 {
                (mem as f32 / max as f32) * 100.0
            } else {
                0.0
            };
            stat.ram.push_back(pct);
            if stat.ram.len() > 48 {
                stat.ram.pop_front();
            }
        }
        let names: Vec<String> = self.vms.iter().map(|v| v.name.clone()).collect();
        self.stats.retain(|key, _| names.iter().any(|n| n == key));
    }

    fn unique_name(&self, base: &str) -> String {
        let mut name = base.to_string();
        let mut i = 2;
        while self.vms.iter().any(|v| v.name == name) {
            name = format!("{base}-{i}");
            i += 1;
        }
        name
    }

    fn form_sync_name(&mut self) {
        let profile = osdata::find(&self.form.profile_id);
        let base = osdata::auto_name(profile, &self.form.version);
        let base = if base.is_empty() {
            "vm".to_string()
        } else {
            base
        };
        self.form.name = self.unique_name(&base);
    }

    fn form_select_profile(&mut self, id: &'static str, cx: &mut Context<Self>) {
        let profile = osdata::find(id);
        self.form.profile_id = profile.id.to_string();
        self.form.version.clear();
        self.form
            .defaults(profile, self.host_ram_mb, self.host_cpus, self.kvm_ok);
        self.form_sync_name();
        cx.notify();
    }

    fn form_select_iso(&mut self, idx: Option<usize>, cx: &mut Context<Self>) {
        self.form.iso = idx;
        if let Some(path) = idx.and_then(|i| self.form.isos.get(i)) {
            let (profile, version) = osdata::detect_from_iso(&path.to_string_lossy());
            self.form.profile_id = profile.id.to_string();
            self.form.version = version;
            self.form
                .defaults(profile, self.host_ram_mb, self.host_cpus, self.kvm_ok);
        } else {
            self.form.profile_id = "other".into();
            self.form.version.clear();
            let profile = osdata::find("other");
            self.form
                .defaults(profile, self.host_ram_mb, self.host_cpus, self.kvm_ok);
        }
        self.form_sync_name();
        cx.notify();
    }

    fn form_rescan(&mut self, cx: &mut Context<Self>) {
        self.form.isos = backend::iso_files();
        if self
            .form
            .iso
            .map(|i| i >= self.form.isos.len())
            .unwrap_or(false)
        {
            self.form.iso = None;
        }
        self.set_status(
            format!(
                "{} ISO file(s) found in ~/VirtualMachines/iso",
                self.form.isos.len()
            ),
            StatusKind::Info,
        );
        cx.notify();
    }

    fn form_step(&mut self, field: HwField, delta: i32, cx: &mut Context<Self>) {
        match field {
            HwField::Vcpu => {
                self.form.vcpu =
                    (self.form.vcpu as i32 + delta).clamp(1, self.host_cpus.max(1) as i32) as u32;
            }
            HwField::Ram => {
                self.form.ram = (self.form.ram as i32 + delta * 512)
                    .clamp(256, self.host_ram_mb.max(256) as i32)
                    as u32;
            }
        }
        cx.notify();
    }

    fn form_disk_step(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.form.disk = (self.form.disk as i32 + delta * 5).clamp(5, 2000) as u32;
        cx.notify();
    }

    fn create_vm(&mut self, cx: &mut Context<Self>) {
        let Some(b) = self.backend.as_ref() else {
            self.set_status("Not connected to libvirt", StatusKind::Error);
            cx.notify();
            return;
        };
        let name = self.form.name.clone();
        let profile = osdata::find(&self.form.profile_id);
        let iso = self
            .form
            .iso
            .and_then(|i| self.form.isos.get(i))
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let (vcpu, ram, disk, kvm) = (self.form.vcpu, self.form.ram, self.form.disk, self.form.kvm);
        match b.create_vm(&name, profile, vcpu, ram, disk, &iso, kvm) {
            Ok(()) => {
                self.selected = Some(name.clone());
                self.screen = Screen::Vms;
                self.tab = Tab::Overview;
                self.refresh();
                self.load_details();
                self.set_status(format!("Created {name}"), StatusKind::Ok);
            }
            Err(e) => self.set_status(format!("Create failed: {e}"), StatusKind::Error),
        }
        cx.notify();
    }

    fn vm_action(&mut self, action: VmAction, cx: &mut Context<Self>) {
        let Some(name) = self.selected.clone() else {
            return;
        };
        let res = {
            let Some(b) = self.backend.as_ref() else {
                self.set_status("Not connected to libvirt", StatusKind::Error);
                cx.notify();
                return;
            };
            match action {
                VmAction::Start => b.start(&name),
                VmAction::Shutdown => b.shutdown(&name),
                VmAction::ForceOff => b.force_off(&name),
                VmAction::Reboot => b.reboot(&name),
                VmAction::Pause => b.pause(&name),
                VmAction::Resume => b.resume(&name),
                VmAction::Console => b.console(&name),
            }
        };
        match res {
            Ok(()) => {
                let msg = match action {
                    VmAction::Start => format!("Starting {name}"),
                    VmAction::Shutdown => format!("Shutdown requested for {name}"),
                    VmAction::ForceOff => format!("{name} powered off"),
                    VmAction::Reboot => format!("Rebooting {name}"),
                    VmAction::Pause => format!("{name} paused"),
                    VmAction::Resume => format!("{name} resumed"),
                    VmAction::Console => "Console launched".to_string(),
                };
                self.set_status(msg, StatusKind::Ok);
                if !matches!(action, VmAction::Console) {
                    self.refresh();
                    self.load_details();
                }
            }
            Err(e) => self.set_status(format!("{e}"), StatusKind::Error),
        }
        cx.notify();
    }

    fn delete_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = self.selected.clone() else {
            return;
        };
        let running = self
            .selected_vm()
            .map(|vm| vm.running() || vm.paused())
            .unwrap_or(false);
        if running {
            self.set_status(
                "Stop the virtual machine before deleting it",
                StatusKind::Error,
            );
            cx.notify();
            return;
        }
        let answer = window.prompt(
            PromptLevel::Warning,
            &format!("Delete {name}?"),
            Some("The VM definition and NVRAM will be removed. Disk files stay on disk."),
            &["Delete", "Cancel"],
            &mut *cx,
        );
        cx.spawn(async move |this, cx| {
            if let Ok(0) = answer.await {
                let _ = this.update(cx, |this, cx| {
                    this.delete_selected_now(&name, cx);
                });
            }
        })
        .detach();
    }

    fn delete_selected_now(&mut self, name: &str, cx: &mut Context<Self>) {
        let res = {
            let Some(b) = self.backend.as_ref() else {
                return;
            };
            b.delete(name)
        };
        match res {
            Ok(()) => {
                self.stats.remove(name);
                self.selected = None;
                self.hw = None;
                self.snaps.clear();
                self.snap_arm = None;
                self.refresh();
                self.set_status(format!("Deleted {name}"), StatusKind::Ok);
            }
            Err(e) => self.set_status(format!("Delete failed: {e}"), StatusKind::Error),
        }
        cx.notify();
    }

    fn hw_step(&mut self, field: HwField, delta: i32, cx: &mut Context<Self>) {
        if let Some(hw) = self.hw.as_mut() {
            match field {
                HwField::Vcpu => {
                    hw.vcpu =
                        (hw.vcpu as i32 + delta).clamp(1, self.host_cpus.max(1) as i32) as u32;
                }
                HwField::Ram => {
                    hw.ram = (hw.ram as i32 + delta * 512)
                        .clamp(256, self.host_ram_mb.max(256) as i32)
                        as u32;
                }
            }
            cx.notify();
        }
    }

    fn hw_toggle_kvm(&mut self, cx: &mut Context<Self>) {
        if let Some(hw) = self.hw.as_mut() {
            hw.kvm = !hw.kvm;
            cx.notify();
        }
    }

    fn hw_toggle_uefi(&mut self, cx: &mut Context<Self>) {
        if let Some(hw) = self.hw.as_mut() {
            hw.uefi = !hw.uefi;
            if hw.uefi {
                hw.machine = "q35".into();
            }
            cx.notify();
        }
    }

    fn hw_toggle_tpm(&mut self, cx: &mut Context<Self>) {
        if let Some(hw) = self.hw.as_mut() {
            hw.tpm = !hw.tpm;
            cx.notify();
        }
    }

    fn hw_toggle_autostart(&mut self, cx: &mut Context<Self>) {
        if let Some(hw) = self.hw.as_mut() {
            hw.autostart = !hw.autostart;
            cx.notify();
        }
    }

    fn hw_cycle_video(&mut self, cx: &mut Context<Self>) {
        if let Some(hw) = self.hw.as_mut() {
            hw.video = match hw.video.as_str() {
                "qxl" => "virtio".into(),
                "virtio" => "vga".into(),
                _ => "qxl".into(),
            };
            cx.notify();
        }
    }

    fn hw_apply(&mut self, cx: &mut Context<Self>) {
        let Some(name) = self.selected.clone() else {
            return;
        };
        let Some(hw) = self.hw.clone() else { return };
        let res = {
            let Some(b) = self.backend.as_ref() else {
                return;
            };
            b.hw_apply(&name, &hw)
        };
        match res {
            Ok(deferred) => {
                if deferred {
                    self.set_status(
                        "Saved. RAM and vCPU changes take effect after the next start.",
                        StatusKind::Info,
                    );
                } else {
                    self.set_status("Hardware updated", StatusKind::Ok);
                }
                self.refresh();
                self.load_details();
            }
            Err(e) => self.set_status(format!("Apply failed: {e}"), StatusKind::Error),
        }
        cx.notify();
    }

    fn snap_create(&mut self, cx: &mut Context<Self>) {
        let Some(vmname) = self.selected.clone() else {
            return;
        };
        let mut idx = 1;
        let snapname = loop {
            let candidate = format!("snap-{idx}");
            if !self.snaps.iter().any(|s| s.name == candidate) {
                break candidate;
            }
            idx += 1;
        };
        let res = {
            let Some(b) = self.backend.as_ref() else {
                return;
            };
            b.snap_create(&vmname, &snapname, "Created by VMM-KVM")
        };
        match res {
            Ok(()) => {
                self.reload_snaps();
                self.set_status(format!("Snapshot {snapname} created"), StatusKind::Ok);
            }
            Err(e) => self.set_status(format!("Snapshot failed: {e}"), StatusKind::Error),
        }
        cx.notify();
    }

    fn snap_action(&mut self, action: SnapArm, cx: &mut Context<Self>) {
        if self.snap_arm.as_ref() != Some(&action) {
            self.snap_arm = Some(action);
            cx.notify();
            return;
        }
        let Some(vmname) = self.selected.clone() else {
            return;
        };
        let res = {
            let Some(b) = self.backend.as_ref() else {
                return;
            };
            match &action {
                SnapArm::Revert(snap) => b.snap_revert(&vmname, snap),
                SnapArm::Delete(snap) => b.snap_delete(&vmname, snap),
            }
        };
        self.snap_arm = None;
        match res {
            Ok(()) => {
                let msg = match &action {
                    SnapArm::Revert(snap) => format!("Reverted to {snap}"),
                    SnapArm::Delete(snap) => format!("Deleted snapshot {snap}"),
                };
                self.set_status(msg, StatusKind::Ok);
                self.reload_snaps();
                self.refresh();
            }
            Err(e) => self.set_status(format!("Snapshot operation failed: {e}"), StatusKind::Error),
        }
        cx.notify();
    }

    fn nav(&mut self, screen: Screen, cx: &mut Context<Self>) {
        if screen == Screen::Create {
            self.form_sync_name();
        }
        self.screen = screen;
        cx.notify();
    }

    fn select_vm(&mut self, name: String, cx: &mut Context<Self>) {
        self.selected = Some(name);
        self.snap_arm = None;
        self.load_details();
        cx.notify();
    }

    fn clear_status(&mut self, cx: &mut Context<Self>) {
        self.status = None;
        cx.notify();
    }

    fn screen_text(&self) -> (&'static str, String) {
        let running = self.vms.iter().filter(|v| v.running()).count();
        match self.screen {
            Screen::Vms => (
                "Virtual Machines",
                format!("{} total · {} running", self.vms.len(), running),
            ),
            Screen::Create => (
                "Create Virtual Machine",
                "One screen, sensible defaults".into(),
            ),
            Screen::Storage => ("Storage Pools", "libvirt storage volumes".into()),
            Screen::Networks => ("Networks", "Virtual networks and bridges".into()),
            Screen::Ssh => ("SSH Hosts", "Remote hypervisors and consoles".into()),
            Screen::Settings => ("Settings", "Application preferences".into()),
        }
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> Div {
        let conn = match &self.backend {
            Some(b) => {
                let label = if b.is_session() { "session" } else { "system" };
                format!("libvirt · {label}")
            }
            None => "offline".to_string(),
        };
        let conn_color = if self.backend.is_some() {
            col(GREEN)
        } else {
            col(RED)
        };

        div()
            .w(px(216.))
            .h_full()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .bg(col(PANEL))
            .border_r_1()
            .border_color(col(BORDER))
            .p(px(12.))
            .gap(px(4.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(6.))
                    .pt(px(4.))
                    .pb(px(14.))
                    .child(
                        div()
                            .size(px(32.))
                            .rounded(px(8.))
                            .bg(col(ACCENT))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(col(0x062a26))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(16.))
                            .child("V"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::BOLD)
                                    .child("VMM-KVM"),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(col(MUTED))
                                    .child("QEMU / KVM manager"),
                            ),
                    ),
            )
            .child(self.nav_item("nav_vms", "Virtual Machines", "▣", Screen::Vms, cx))
            .child(self.nav_item(
                "nav_create",
                "New Virtual Machine",
                "＋",
                Screen::Create,
                cx,
            ))
            .child(div().h(px(10.)))
            .child(self.nav_item("nav_storage", "Storage Pools", "▤", Screen::Storage, cx))
            .child(self.nav_item("nav_networks", "Networks", "⇄", Screen::Networks, cx))
            .child(self.nav_item("nav_ssh", "SSH Hosts", "⌗", Screen::Ssh, cx))
            .child(self.nav_item("nav_settings", "Settings", "⚙", Screen::Settings, cx))
            .child(div().flex_1())
            .child(
                div()
                    .p(px(12.))
                    .rounded(px(10.))
                    .bg(col(CARD))
                    .border_1()
                    .border_color(col(BORDER))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(px(9.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(col(MUTED))
                            .child("HYPERVISOR"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(dot(conn_color, 6.0))
                            .child(div().text_size(px(12.)).child(conn)),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(col(MUTED))
                            .child(format!(
                                "{} CPU · {} RAM",
                                self.host_cpus,
                                fmt_mb(self.host_ram_mb)
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(dot(if self.kvm_ok { col(GREEN) } else { col(AMBER) }, 6.0))
                            .child(div().text_size(px(11.)).text_color(col(MUTED)).child(
                                if self.kvm_ok {
                                    "KVM available"
                                } else {
                                    "KVM unavailable"
                                },
                            )),
                    ),
            )
            .child(
                div()
                    .pt(px(8.))
                    .truncate()
                    .text_size(px(10.))
                    .text_color(col(MUTED))
                    .child(format!("vmm-kvm-rs {}", env!("CARGO_PKG_VERSION"))),
            )
    }

    fn nav_item(
        &self,
        id: &'static str,
        label: &'static str,
        glyph: &'static str,
        screen: Screen,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let active = self.screen == screen;
        div()
            .id(id)
            .flex()
            .items_center()
            .gap(px(10.))
            .h(px(38.))
            .px(px(10.))
            .rounded(px(8.))
            .border_l_2()
            .border_color(if active { col(ACCENT) } else { col(PANEL) })
            .bg(if active { col(CARD2) } else { col(PANEL) })
            .text_color(if active { col(ACCENT) } else { col(MUTED) })
            .text_size(px(13.))
            .cursor_pointer()
            .hover(|this| this.bg(col(CARD)).text_color(col(TEXT)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.nav(screen, cx)))
            .child(div().w(px(18.)).flex().justify_center().child(glyph))
            .child(label)
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> Div {
        let (title, subtitle) = self.screen_text();
        div()
            .h(px(64.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .px(px(20.))
            .bg(col(PANEL))
            .border_b_1()
            .border_color(col(BORDER))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .child(
                        div()
                            .text_size(px(18.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(col(MUTED))
                            .child(subtitle),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(7.))
                            .px(px(10.))
                            .h(px(30.))
                            .rounded(px(7.))
                            .bg(col(CARD))
                            .border_1()
                            .border_color(col(BORDER))
                            .child(dot(
                                if self.backend.is_some() {
                                    col(GREEN)
                                } else {
                                    col(RED)
                                },
                                7.0,
                            ))
                            .child(div().text_size(px(11.)).text_color(col(MUTED)).child(
                                if self.backend.is_some() {
                                    "connected"
                                } else {
                                    "offline"
                                },
                            )),
                    )
                    .child(
                        button("tb_refresh", "Refresh", BtnKind::Normal).on_click(cx.listener(
                            |this, _: &ClickEvent, _, cx| {
                                this.refresh();
                                this.set_status("Refreshed", StatusKind::Info);
                                cx.notify();
                            },
                        )),
                    )
                    .child(button("tb_new", "＋ New VM", BtnKind::Accent).on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| this.nav(Screen::Create, cx)),
                    )),
            )
    }

    fn body(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.backend.is_none() {
            return self.connection_error(cx).into_any_element();
        }
        match self.screen {
            Screen::Vms => self.vms_screen(cx).into_any_element(),
            Screen::Create => self.create_screen(cx).into_any_element(),
            Screen::Storage => self.placeholder(
                "▤",
                "Storage pools",
                "Manage libvirt storage pools and volumes here. The core build only manages VM disks in ~/VirtualMachines/vms.",
                cx,
            ).into_any_element(),
            Screen::Networks => self.placeholder(
                "⇄",
                "Virtual networks",
                "Browse libvirt networks and bridges here. Session VMs use user-mode networking for zero configuration.",
                cx,
            ).into_any_element(),
            Screen::Ssh => self.placeholder(
                "⌗",
                "SSH hosts",
                "Connect to remote libvirt daemons over SSH. This screen is planned for the next build.",
                cx,
            ).into_any_element(),
            Screen::Settings => self.placeholder(
                "⚙",
                "Settings",
                "Language, default paths and console preferences will live here. The core build keeps things simple.",
                cx,
            ).into_any_element(),
        }
    }

    fn connection_error(&self, cx: &mut Context<Self>) -> Div {
        let detail = self.conn_err.clone().unwrap_or_default();
        div()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .items_center()
            .justify_center()
            .p(px(24.))
            .child(
                div()
                    .w(px(520.))
                    .p(px(26.))
                    .rounded(px(12.))
                    .bg(col(PANEL))
                    .border_1()
                    .border_color(col(BORDER))
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(div().text_size(px(30.)).child("⚠"))
                    .child(
                        div()
                            .text_size(px(18.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Cannot connect to libvirt"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(col(MUTED))
                            .child("Install libvirt, qemu and virt-viewer, then make sure the libvirt session daemon can start."),
                    )
                    .child(
                        div()
                            .p(px(10.))
                            .rounded(px(8.))
                            .bg(col(CARD))
                            .text_size(px(11.))
                            .text_color(col(RED))
                            .child(detail),
                    )
                    .child(
                        div().flex().gap(px(8.)).child(
                            button("retry_conn", "Retry connection", BtnKind::Accent)
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.reconnect(cx)
                                })),
                        ),
                    ),
            )
    }

    fn vms_screen(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .flex_row()
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .flex()
                    .flex_col()
                    .p(px(16.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .pb(px(12.))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(col(MUTED))
                                    .child(format!(
                                        "{} machine(s), {} running",
                                        self.vms.len(),
                                        self.vms.iter().filter(|v| v.running()).count()
                                    )),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(col(MUTED))
                                    .child("click a card to inspect"),
                            ),
                    )
                    .child(
                        div()
                            .id("vm_list")
                            .flex_1()
                            .min_h(px(0.))
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .children(self.vms.iter().map(|vm| self.vm_card(vm, cx))),
                    ),
            )
            .child(self.details_panel(cx))
    }

    fn vm_card(&self, vm: &Vm, cx: &mut Context<Self>) -> Stateful<Div> {
        let profile = vm.os();
        let selected = self.selected.as_deref() == Some(vm.name.as_str());
        let name = vm.name.clone();
        div()
            .id(SharedString::from(format!("vm-{}", vm.name)))
            .flex()
            .items_center()
            .gap(px(14.))
            .p(px(14.))
            .rounded(px(10.))
            .bg(col(if selected { CARD2 } else { CARD }))
            .border_1()
            .border_color(col(if selected { ACCENT } else { BORDER }))
            .cursor_pointer()
            .hover(|this| this.border_color(col(ACCENT_D)))
            .on_click(
                cx.listener(move |this, _: &ClickEvent, _, cx| this.select_vm(name.clone(), cx)),
            )
            .child(os_icon(profile.icon, 34.0))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(vm.name.clone()),
                            )
                            .child(state_badge(vm)),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(col(MUTED))
                            .child(format!(
                                "{} · {} vCPU · {} RAM · {} disk",
                                profile.name,
                                vm.vcpus,
                                fmt_mb(vm.maxmem_mb),
                                fmt_disk(vm.disk_gb)
                            )),
                    ),
            )
            .child(self.stats_block(vm))
    }

    fn stats_block(&self, vm: &Vm) -> Div {
        let stat = self.stats.get(&vm.name);
        let cpu_hist = stat.map(|s| &s.cpu);
        let ram_hist = stat.map(|s| &s.ram);
        let cpu_last = cpu_hist
            .and_then(|h| h.back().copied())
            .filter(|_| vm.running())
            .unwrap_or(0.0);
        let ram_last = ram_hist
            .and_then(|h| h.back().copied())
            .filter(|_| vm.running())
            .unwrap_or(0.0);
        div()
            .w(px(208.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(px(5.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .w(px(30.))
                            .text_size(px(9.))
                            .text_color(col(MUTED))
                            .child("CPU"),
                    )
                    .child(spark(cpu_hist, col(ACCENT)))
                    .child(
                        div()
                            .w(px(36.))
                            .text_size(px(9.))
                            .text_color(col(if cpu_last > 70.0 { AMBER } else { TEXT }))
                            .child(format!("{cpu_last:.0}%")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .w(px(30.))
                            .text_size(px(9.))
                            .text_color(col(MUTED))
                            .child("RAM"),
                    )
                    .child(spark(ram_hist, col(GREEN)))
                    .child(
                        div()
                            .w(px(36.))
                            .text_size(px(9.))
                            .text_color(col(if ram_last > 90.0 { AMBER } else { TEXT }))
                            .child(format!("{ram_last:.0}%")),
                    ),
            )
    }

    fn details_panel(&self, cx: &mut Context<Self>) -> Div {
        div()
            .w(px(388.))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(col(PANEL))
            .border_l_1()
            .border_color(col(BORDER))
            .child(
                div()
                    .flex()
                    .border_b_1()
                    .border_color(col(BORDER))
                    .child(self.tab_button(Tab::Overview, "Overview", cx))
                    .child(self.tab_button(Tab::Hardware, "Hardware", cx))
                    .child(self.tab_button(Tab::Snapshots, "Snapshots", cx)),
            )
            .child(
                div()
                    .id("details_scroll")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .p(px(16.))
                    .child(match self.tab {
                        Tab::Overview => self.overview_tab(cx),
                        Tab::Hardware => self.hardware_tab(cx),
                        Tab::Snapshots => self.snapshots_tab(cx),
                    }),
            )
    }

    fn tab_button(&self, tab: Tab, label: &'static str, cx: &mut Context<Self>) -> Stateful<Div> {
        let active = self.tab == tab;
        div()
            .id(match tab {
                Tab::Overview => "tab_overview",
                Tab::Hardware => "tab_hardware",
                Tab::Snapshots => "tab_snapshots",
            })
            .flex_1()
            .flex()
            .justify_center()
            .py(px(11.))
            .border_b_1()
            .border_color(if active { col(ACCENT) } else { col(BORDER) })
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(if active { col(ACCENT) } else { col(MUTED) })
            .cursor_pointer()
            .hover(|this| this.text_color(col(TEXT)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.tab = tab;
                cx.notify();
            }))
            .child(label)
    }

    fn overview_tab(&self, cx: &mut Context<Self>) -> Div {
        let Some(vm) = self.selected_vm() else {
            return self.no_selection();
        };
        let profile = vm.os();
        let hw = self.hw.as_ref();
        let vcpu_value = if vm.running() && vm.live_vcpus != vm.vcpus {
            format!("{} (now {})", vm.vcpus, vm.live_vcpus)
        } else {
            vm.vcpus.to_string()
        };
        let ram_value = if vm.running() && vm.live_maxmem_mb != vm.maxmem_mb {
            format!(
                "{} (now {})",
                fmt_mb(vm.maxmem_mb),
                fmt_mb(vm.live_maxmem_mb)
            )
        } else {
            fmt_mb(vm.maxmem_mb)
        };
        let machine = hw.map(|h| h.machine.clone()).unwrap_or_else(|| "—".into());
        let uefi = hw
            .map(|h| if h.uefi { "UEFI" } else { "BIOS" })
            .unwrap_or("—");
        let tpm = hw.map(|h| if h.tpm { "yes" } else { "no" }).unwrap_or("—");
        let video = hw.map(|h| h.video.clone()).unwrap_or_else(|| "—".into());

        let mut actions: Vec<Stateful<Div>> = Vec::new();
        if vm.running() {
            actions.push(self.action_button(
                "act_shutdown",
                "Shut down",
                BtnKind::Normal,
                VmAction::Shutdown,
                cx,
            ));
            actions.push(self.action_button(
                "act_force",
                "Force off",
                BtnKind::Danger,
                VmAction::ForceOff,
                cx,
            ));
            actions.push(self.action_button(
                "act_reboot",
                "Reboot",
                BtnKind::Normal,
                VmAction::Reboot,
                cx,
            ));
            actions.push(self.action_button(
                "act_pause",
                "Pause",
                BtnKind::Normal,
                VmAction::Pause,
                cx,
            ));
        } else if vm.paused() {
            actions.push(self.action_button(
                "act_resume2",
                "Resume",
                BtnKind::Accent,
                VmAction::Resume,
                cx,
            ));
            actions.push(self.action_button(
                "act_force2",
                "Force off",
                BtnKind::Danger,
                VmAction::ForceOff,
                cx,
            ));
        } else {
            actions.push(self.action_button(
                "act_start",
                "Start",
                BtnKind::Accent,
                VmAction::Start,
                cx,
            ));
        }
        actions.push(self.action_button(
            "act_console",
            "Console",
            BtnKind::Normal,
            VmAction::Console,
            cx,
        ));

        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(os_icon(profile.icon, 44.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(px(17.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(vm.name.clone()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(state_badge(vm))
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(col(MUTED))
                                            .child(profile.name),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(self.info_row("Operating system", profile.name))
                    .child(self.info_row("State", state_name(vm)))
                    .child(self.info_row("vCPU", vcpu_value))
                    .child(self.info_row("Memory", ram_value))
                    .child(self.info_row("Disk", fmt_disk(vm.disk_gb)))
                    .child(self.info_row("Machine", machine))
                    .child(self.info_row("Firmware", uefi))
                    .child(self.info_row("TPM", tpm))
                    .child(self.info_row("Video", video))
                    .child(self.info_row("Autostart", if vm.autostart { "on" } else { "off" }))
                    .child(self.info_row("Console", "SPICE")),
            )
            .child(
                div()
                    .text_size(px(10.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(col(MUTED))
                    .child("POWER"),
            )
            .child(div().flex().flex_wrap().gap(px(8.)).children(actions))
            .child(div().h(px(4.)))
            .child(
                div()
                    .text_size(px(10.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(col(MUTED))
                    .child("DANGER ZONE"),
            )
            .child(
                div().w_full().flex().child(
                    button("act_delete", "Delete virtual machine", BtnKind::Danger)
                        .flex_1()
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.delete_selected(window, cx)
                        })),
                ),
            )
    }

    fn action_button(
        &self,
        id: &'static str,
        label: &str,
        kind: BtnKind,
        action: VmAction,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        button(id, label, kind)
            .w(px(108.))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.vm_action(action, cx)))
    }

    fn info_row(&self, label: &str, value: impl Into<SharedString>) -> Div {
        div()
            .flex()
            .justify_between()
            .items_center()
            .py(px(7.))
            .border_b_1()
            .border_color(col(BORDER))
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(col(MUTED))
                    .child(label.to_string()),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .child(value.into()),
            )
    }

    fn snapshots_tab(&self, cx: &mut Context<Self>) -> Div {
        let Some(vm) = self.selected_vm() else {
            return self.no_selection();
        };
        let mut rows: Vec<Div> = Vec::new();
        for snap in &self.snaps {
            let revert_armed = self.snap_arm == Some(SnapArm::Revert(snap.name.clone()));
            let delete_armed = self.snap_arm == Some(SnapArm::Delete(snap.name.clone()));
            let subtitle = if snap.desc.is_empty() {
                fmt_time(&snap.created)
            } else {
                format!("{} · {}", snap.desc, fmt_time(&snap.created))
            };
            let revert_name = snap.name.clone();
            let delete_name = snap.name.clone();
            rows.push(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .p(px(10.))
                    .rounded(px(9.))
                    .bg(col(CARD))
                    .border_1()
                    .border_color(col(BORDER))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .truncate()
                                    .child(snap.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(9.))
                                    .text_color(col(MUTED))
                                    .truncate()
                                    .child(subtitle),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                button(
                                    "snap_revert",
                                    if revert_armed { "Confirm" } else { "Revert" },
                                    if revert_armed {
                                        BtnKind::Accent
                                    } else {
                                        BtnKind::Normal
                                    },
                                )
                                .h(px(26.))
                                .on_click(cx.listener(
                                    move |this, _: &ClickEvent, _, cx| {
                                        this.snap_action(SnapArm::Revert(revert_name.clone()), cx)
                                    },
                                )),
                            )
                            .child(
                                button(
                                    "snap_delete",
                                    if delete_armed { "Confirm" } else { "Delete" },
                                    BtnKind::Danger,
                                )
                                .h(px(26.))
                                .on_click(cx.listener(
                                    move |this, _: &ClickEvent, _, cx| {
                                        this.snap_action(SnapArm::Delete(delete_name.clone()), cx)
                                    },
                                )),
                            ),
                    ),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Snapshots"),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(col(MUTED))
                                    .child(format!("{} snapshot(s) · {}", self.snaps.len(), vm.name)),
                            ),
                    )
                    .child(
                        button("snap_create", "Create snapshot", BtnKind::Accent)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.snap_create(cx))),
                    ),
            )
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(col(if vm.running() { AMBER } else { MUTED }))
                    .child(if vm.running() {
                        "Stop the VM before reverting. Creating a snapshot of a running VM requires qcow2 disks."
                    } else {
                        "Snapshots capture the disk state. Revert restores it; delete removes only the snapshot."
                    }),
            )
            .children(rows)
            .child(if self.snaps.is_empty() {
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(4.))
                    .py(px(30.))
                    .child(div().text_size(px(24.)).text_color(col(MUTED)).child("⟲"))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .child("No snapshots yet"),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(col(MUTED))
                            .child("Create one to freeze the current disk state."),
                    )
            } else {
                div()
            })
    }

    fn no_selection(&self) -> Div {
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .py(px(60.))
            .child(div().text_size(px(28.)).text_color(col(MUTED)).child("▣"))
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .child("No virtual machine selected"),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(col(MUTED))
                    .child("Select a machine on the left or create a new one."),
            )
    }

    fn hardware_tab(&self, cx: &mut Context<Self>) -> Div {
        let Some(vm) = self.selected_vm() else {
            return self.no_selection();
        };
        let Some(hw) = self.hw.as_ref() else {
            return div()
                .text_size(px(12.))
                .text_color(col(MUTED))
                .child("Hardware information is unavailable for this machine.");
        };
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Hardware"),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(col(MUTED))
                            .child(format!(
                                "Saved to the VM configuration. Running machines keep their current resources until the next start ({}).",
                                vm.name
                            )),
                    ),
            )
            .child(
                div()
                    .p(px(12.))
                    .rounded(px(9.))
                    .bg(col(CARD))
                    .border_1()
                    .border_color(col(BORDER))
                    .flex()
                    .flex_col()
                    .child(self.hw_stepper(
                        "vCPU",
                        hw.vcpu.to_string(),
                        &format!("1 – {} (host)", self.host_cpus),
                        "hw_vcpu_minus",
                        "hw_vcpu_plus",
                        HwField::Vcpu,
                        cx,
                    ))
                    .child(self.hw_stepper(
                        "Memory",
                        fmt_mb(hw.ram),
                        &format!("256 MiB – {} (host)", fmt_mb(self.host_ram_mb)),
                        "hw_ram_minus",
                        "hw_ram_plus",
                        HwField::Ram,
                        cx,
                    ))
                    .child(
                        toggle_row(
                            "KVM acceleration",
                            if self.kvm_ok { "Hardware virtualization" } else { "KVM is not available on this host" },
                            hw.kvm,
                        )
                        .id("hw_kvm")
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.hw_toggle_kvm(cx))),
                    )
                    .child(
                        toggle_row("UEFI firmware", "q35 machine is selected automatically", hw.uefi)
                            .id("hw_uefi")
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.hw_toggle_uefi(cx))),
                    )
                    .child(
                        toggle_row("TPM 2.0", "Required by Windows 11", hw.tpm)
                            .id("hw_tpm")
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.hw_toggle_tpm(cx))),
                    )
                    .child(
                        toggle_row("Autostart", "Start with the libvirt daemon", hw.autostart)
                            .id("hw_autostart")
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.hw_toggle_autostart(cx))),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .py(px(12.))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child("Video model"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(col(MUTED))
                                            .child("qxl → virtio → vga"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(col(ACCENT))
                                            .child(hw.video.clone()),
                                    )
                                    .child(
                                        step_button("hw_video", "Change")
                                            .w(px(64.))
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.hw_cycle_video(cx)
                                            })),
                                    ),
                            ),
                    ),
            )
            .child(
                div().w_full().flex().child(
                    button("hw_apply", "Apply hardware changes", BtnKind::Accent)
                        .h(px(34.))
                        .flex_1()
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.hw_apply(cx))),
                ),
            )
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(col(MUTED))
                    .child("Machine, chipset and disk devices are preserved. RAM/vCPU hot-plug depends on the guest."),
            )
    }

    fn hw_stepper(
        &self,
        label: &str,
        value: String,
        hint: &str,
        minus_id: &'static str,
        plus_id: &'static str,
        field: HwField,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex()
            .items_center()
            .justify_between()
            .py(px(10.))
            .border_b_1()
            .border_color(col(BORDER))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(label.to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(col(MUTED))
                            .child(hint.to_string()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(step_button(minus_id, "−").on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| this.hw_step(field, -1, cx)),
                    ))
                    .child(
                        div()
                            .w(px(78.))
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(col(TEXT))
                            .child(value),
                    )
                    .child(step_button(plus_id, "＋").on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| this.hw_step(field, 1, cx)),
                    )),
            )
    }

    fn create_screen(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let mut iso_rows: Vec<Stateful<Div>> = Vec::new();
        iso_rows.push(self.iso_row(
            None,
            "No installation media",
            "Create an empty VM and install later",
            "other",
            cx,
        ));
        for (i, path) in self.form.isos.iter().enumerate() {
            let file = path
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string());
            let profile = osdata::detect_from_iso(&path.to_string_lossy()).0;
            iso_rows.push(self.iso_row(
                Some(i),
                &file,
                &format!("{} · ~/VirtualMachines/iso", profile.name),
                profile.icon,
                cx,
            ));
        }

        let mut grid_rows: Vec<Div> = Vec::new();
        for chunk in osdata::OS_LIST.chunks(4) {
            let mut row_chips: Vec<Stateful<Div>> = Vec::new();
            for profile in chunk {
                let id = profile.id;
                let selected = self.form.profile_id == profile.id;
                row_chips.push(
                    div()
                        .id(SharedString::from(format!("os-{}", profile.id)))
                        .w(px(172.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .px(px(10.))
                        .py(px(8.))
                        .rounded(px(8.))
                        .bg(col(if selected { CARD2 } else { CARD }))
                        .border_1()
                        .border_color(col(if selected { ACCENT } else { BORDER }))
                        .cursor_pointer()
                        .hover(|this| this.border_color(col(ACCENT_D)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.form_select_profile(id, cx)
                        }))
                        .child(os_icon(profile.icon, 18.0))
                        .child(div().text_size(px(11.)).truncate().child(profile.name)),
                );
            }
            grid_rows.push(div().flex().gap(px(8.)).children(row_chips));
        }

        let total_cpu = self.host_cpus;
        let total_ram = self.host_ram_mb;

        div()
            .flex_1()
            .min_h(px(0.))
            .id("create_scroll")
            .overflow_y_scroll()
            .p(px(20.))
            .child(
                div()
                    .w_full()
                    .max_w(px(800.))
                    .mx_auto()
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(3.))
                                    .child(
                                        div()
                                            .text_size(px(20.))
                                            .font_weight(FontWeight::BOLD)
                                            .child("Create a virtual machine"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(col(MUTED))
                                            .child("Pick install media, confirm the detected OS and adjust hardware."),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(col(MUTED))
                                            .child(format!("host: {} CPU · {}", total_cpu, fmt_mb(total_ram))),
                                    )
                                    .child(
                                        button("iso_rescan", "Rescan ISOs", BtnKind::Normal)
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.form_rescan(cx)
                                            })),
                                    ),
                            ),
                    )
                    .child(section_label("INSTALLATION MEDIA"))
                    .child(
                        div()
                            .id("iso_list")
                            .max_h(px(158.))
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .children(iso_rows),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(col(MUTED))
                            .child("Place .iso files in ~/VirtualMachines/iso, then press Rescan ISOs."),
                    )
                    .child(section_label("OPERATING SYSTEM"))
                    .child(div().flex().flex_col().gap(px(8.)).children(grid_rows))
                    .child(section_label("NAME"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(12.))
                            .h(px(40.))
                            .rounded(px(8.))
                            .bg(col(CARD))
                            .border_1()
                            .border_color(col(BORDER))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.))
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(col(MUTED))
                                            .child("name"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(col(TEXT))
                                            .child(self.form.name.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(col(MUTED))
                                    .child("auto-generated, unique"),
                            ),
                    )
                    .child(section_label("HARDWARE"))
                    .child(
                        div()
                            .flex()
                            .gap(px(10.))
                            .child(self.create_stepper(
                                "vCPU",
                                self.form.vcpu.to_string(),
                                &format!("max {}", total_cpu),
                                "form_vcpu_minus",
                                "form_vcpu_plus",
                                HwField::Vcpu,
                                cx,
                            ))
                            .child(self.create_stepper(
                                "Memory",
                                fmt_mb(self.form.ram),
                                &format!("max {}", fmt_mb(total_ram)),
                                "form_ram_minus",
                                "form_ram_plus",
                                HwField::Ram,
                                cx,
                            ))
                            .child(
                                div()
                                    .flex_1()
                                    .p(px(12.))
                                    .rounded(px(9.))
                                    .bg(col(CARD))
                                    .border_1()
                                    .border_color(col(BORDER))
                                    .flex()
                                    .flex_col()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child("Disk"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .text_size(px(14.))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .child(format!("{} GiB", self.form.disk)),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(6.))
                                                    .child(
                                                        step_button("form_disk_minus", "−")
                                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                                this.form_disk_step(-1, cx)
                                                            })),
                                                    )
                                                    .child(
                                                        step_button("form_disk_plus", "＋")
                                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                                this.form_disk_step(1, cx)
                                                            })),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(col(MUTED))
                                            .child("qcow2, grows on demand"),
                                    ),
                            ),
                    )
                    .child(
                        toggle_row(
                            "KVM acceleration",
                            if self.kvm_ok { "Recommended when /dev/kvm is available" } else { "KVM is not available on this host" },
                            self.form.kvm,
                        )
                        .id("form_kvm")
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.form.kvm = !this.form.kvm && this.kvm_ok;
                            cx.notify();
                        })),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(col(MUTED))
                            .child(format!(
                                "Disk: ~/VirtualMachines/vms/{}/disk.qcow2 · {}",
                                self.form.name,
                                if self.form.iso.is_some() { "boot from CD-ROM" } else { "empty VM" }
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap(px(8.))
                            .pt(px(2.))
                            .child(
                                button("create_cancel", "Cancel", BtnKind::Normal)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.nav(Screen::Vms, cx)
                                    })),
                            )
                            .child(
                                button("create_go", "Create virtual machine", BtnKind::Accent)
                                    .h(px(34.))
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.create_vm(cx)
                                    })),
                            ),
                    ),
            )
    }

    fn iso_row(
        &self,
        idx: Option<usize>,
        title: &str,
        subtitle: &str,
        icon: &str,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let selected = self.form.iso == idx;
        div()
            .id(SharedString::from(format!(
                "iso-{}",
                idx.map(|i| i.to_string()).unwrap_or_else(|| "none".into())
            )))
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(12.))
            .py(px(8.))
            .rounded(px(8.))
            .bg(col(if selected { CARD2 } else { CARD }))
            .border_1()
            .border_color(col(if selected { ACCENT } else { BORDER }))
            .cursor_pointer()
            .hover(|this| this.border_color(col(ACCENT_D)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.form_select_iso(idx, cx)))
            .child(os_icon(icon, 22.0))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(title.to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(9.))
                            .text_color(col(MUTED))
                            .truncate()
                            .child(subtitle.to_string()),
                    ),
            )
            .child(
                div()
                    .w(px(16.))
                    .h(px(16.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(col(if selected { ACCENT } else { BORDER }))
                    .bg(if selected { col(ACCENT) } else { col(CARD) }),
            )
    }

    fn create_stepper(
        &self,
        label: &str,
        value: String,
        hint: &str,
        minus_id: &'static str,
        plus_id: &'static str,
        field: HwField,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex_1()
            .p(px(12.))
            .rounded(px(9.))
            .bg(col(CARD))
            .border_1()
            .border_color(col(BORDER))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .child(label.to_string()),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(value),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(step_button(minus_id, "−").on_click(cx.listener(
                                move |this, _: &ClickEvent, _, cx| this.form_step(field, -1, cx),
                            )))
                            .child(step_button(plus_id, "＋").on_click(cx.listener(
                                move |this, _: &ClickEvent, _, cx| this.form_step(field, 1, cx),
                            ))),
                    ),
            )
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(col(MUTED))
                    .child(hint.to_string()),
            )
    }

    fn placeholder(
        &self,
        glyph: &'static str,
        title: &'static str,
        body: &'static str,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .items_center()
            .justify_center()
            .p(px(24.))
            .child(
                div()
                    .w(px(460.))
                    .p(px(28.))
                    .rounded(px(12.))
                    .bg(col(PANEL))
                    .border_1()
                    .border_color(col(BORDER))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .text_size(px(32.))
                            .text_color(col(ACCENT))
                            .child(glyph),
                    )
                    .child(
                        div()
                            .text_size(px(18.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(div().text_size(px(12.)).text_color(col(MUTED)).child(body))
                    .child(
                        div().pt(px(6.)).child(
                            button(
                                "placeholder_back",
                                "Back to virtual machines",
                                BtnKind::Accent,
                            )
                            .on_click(
                                cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.nav(Screen::Vms, cx)
                                }),
                            ),
                        ),
                    ),
            )
    }

    fn status_bar(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let (text, color) = match &self.status {
            Some((msg, kind)) => (
                msg.clone(),
                match kind {
                    StatusKind::Info => col(MUTED),
                    StatusKind::Ok => col(GREEN),
                    StatusKind::Error => col(RED),
                },
            ),
            None => ("Ready".to_string(), col(MUTED)),
        };
        div()
            .id("status_bar")
            .h(px(28.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .px(px(16.))
            .bg(col(PANEL))
            .border_t_1()
            .border_color(col(BORDER))
            .cursor_pointer()
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.clear_status(cx)))
            .child(div().text_size(px(10.)).text_color(color).child(text))
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(col(MUTED))
                    .child("GPL-3.0 · Rust port"),
            )
    }
}

impl Render for VmmApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.started {
            self.started = true;
            window.set_window_title("VMM-KVM");
            self.start_tick(cx);
        }
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(col(BG))
            .text_color(col(TEXT))
            .text_size(px(13.))
            .font_family("DejaVu Sans")
            .child(self.sidebar(cx))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(self.toolbar(cx))
                    .child(self.body(cx))
                    .child(self.status_bar(cx)),
            )
    }
}

fn dot(color: Rgba, size_px: f32) -> Div {
    div()
        .w(px(size_px))
        .h(px(size_px))
        .rounded(px(size_px / 2.0))
        .bg(color)
        .flex_shrink_0()
}

fn os_icon(icon: &str, size_px: f32) -> impl IntoElement {
    svg()
        .path(SharedString::from(format!("icons/os/{icon}.svg")))
        .size(px(size_px))
        .flex_shrink_0()
        .text_color(col(TEXT))
}

fn button(id: &'static str, label: &str, kind: BtnKind) -> Stateful<Div> {
    let (bg, fg, border) = match kind {
        BtnKind::Accent => (col(ACCENT), col(0x062a26), col(ACCENT)),
        BtnKind::Danger => (col(CARD), col(TEXT), col(BORDER)),
        BtnKind::Normal => (col(CARD), col(TEXT), col(BORDER)),
    };
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .h(px(30.))
        .px(px(12.))
        .rounded(px(7.))
        .bg(bg)
        .text_color(fg)
        .border_1()
        .border_color(border)
        .text_size(px(12.))
        .font_weight(if kind == BtnKind::Accent {
            FontWeight::BOLD
        } else {
            FontWeight::MEDIUM
        })
        .cursor_pointer()
        .hover(move |this| match kind {
            BtnKind::Accent => this.bg(col(ACCENT_D)).border_color(col(ACCENT_D)),
            BtnKind::Danger => this.border_color(col(RED)).text_color(col(RED)),
            BtnKind::Normal => this.border_color(col(ACCENT_D)).text_color(col(ACCENT)),
        })
        .child(label.to_string())
}

fn step_button(id: &'static str, label: &str) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .size(px(26.))
        .rounded(px(7.))
        .bg(col(CARD2))
        .border_1()
        .border_color(col(BORDER))
        .text_size(px(12.))
        .text_color(col(TEXT))
        .cursor_pointer()
        .hover(|this| this.border_color(col(ACCENT_D)).text_color(col(ACCENT)))
        .child(label.to_string())
}

fn section_label(text: &str) -> Div {
    div()
        .text_size(px(10.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(col(MUTED))
        .child(text.to_string())
}

fn toggle_row(label: &str, hint: &str, on: bool) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .py(px(12.))
        .border_b_1()
        .border_color(col(BORDER))
        .cursor_pointer()
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .text_size(px(10.))
                        .text_color(col(MUTED))
                        .child(hint.to_string()),
                ),
        )
        .child(toggle_pill(on))
}

fn toggle_pill(on: bool) -> Div {
    let base = div()
        .w(px(38.))
        .h(px(21.))
        .rounded(px(11.))
        .p(px(2.))
        .bg(if on { col(ACCENT) } else { col(CARD2) })
        .border_1()
        .border_color(if on { col(ACCENT) } else { col(BORDER) })
        .flex()
        .items_center();
    let knob = div()
        .size(px(15.))
        .rounded(px(8.))
        .bg(if on { col(0x062a26) } else { col(MUTED) });
    if on {
        base.justify_end().child(knob)
    } else {
        base.justify_start().child(knob)
    }
}

fn state_name(vm: &Vm) -> &'static str {
    match vm.state {
        1 => "Running",
        3 => "Paused",
        4 => "Shutting down",
        6 => "Crashed",
        7 => "Suspended",
        _ => "Shut off",
    }
}

fn state_color(vm: &Vm) -> Rgba {
    match vm.state {
        1 => col(GREEN),
        3 => col(AMBER),
        6 => col(RED),
        _ => col(MUTED),
    }
}

fn state_badge(vm: &Vm) -> Div {
    let color = state_color(vm);
    div()
        .flex()
        .items_center()
        .gap(px(5.))
        .px(px(7.))
        .py(px(2.))
        .rounded(px(9.))
        .bg(alpha(color, 0.12))
        .child(dot(color, 5.0))
        .child(
            div()
                .text_size(px(9.))
                .text_color(color)
                .child(state_name(vm)),
        )
}

fn spark(history: Option<&VecDeque<f32>>, color: Rgba) -> Div {
    const BARS: usize = 28;
    let vals: Vec<f32> = history
        .map(|h| h.iter().copied().collect())
        .unwrap_or_default();
    let tail = if vals.len() > BARS {
        &vals[vals.len() - BARS..]
    } else {
        &vals[..]
    };
    let offset = BARS.saturating_sub(tail.len());
    let mut bars = Vec::with_capacity(BARS);
    for i in 0..BARS {
        let v = if i >= offset {
            tail.get(i - offset).copied().unwrap_or(0.0)
        } else {
            0.0
        };
        let height = 2.0 + (v.clamp(0.0, 100.0) / 100.0) * 24.0;
        bars.push(
            div()
                .w(px(3.))
                .h(px(height))
                .rounded(px(1.))
                .bg(alpha(color, if v > 0.0 { 0.85 } else { 0.22 })),
        );
    }
    div()
        .w(px(125.))
        .flex_shrink_0()
        .flex()
        .items_end()
        .gap(px(1.))
        .h(px(28.))
        .children(bars)
}

fn fmt_time(created: &str) -> String {
    let Ok(secs) = created.trim().parse::<u64>() else {
        return created.to_string();
    };
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    let (hour, minute) = (rem / 3600, (rem % 3600) / 60);
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    if month <= 2 {
        year += 1;
    }
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02} UTC")
}
