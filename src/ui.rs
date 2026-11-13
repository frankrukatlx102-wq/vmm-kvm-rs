use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, ClickEvent, Context, CursorStyle, Div, FocusHandle, FontWeight, KeyDownEvent,
    Render, Rgba, SharedString, Stateful, Timer, Window, div, prelude::*, px, rgb, svg,
};

use crate::backend::{self, DiskInfo, Hw, NetInfo, SnapInfo, Vm};
use crate::hostinfo;
use crate::i18n::{self, Lang};
use crate::osdata::{self, OsProfile};

const DISK_BUSES: [&str; 4] = ["virtio", "sata", "ide", "scsi"];
const NIC_MODELS: [&str; 3] = ["virtio", "e1000e", "rtl8139"];

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
    Disks,
    Network,
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
    DangerSolid,
}

#[derive(Clone, Copy)]
enum HwField {
    Vcpu,
    Ram,
}

#[derive(Clone)]
struct ConfirmDialog {
    title: String,
    body: String,
    confirm: String,
    action: ConfirmAction,
    opened_at: Instant,
}

#[derive(Clone)]
enum ConfirmAction {
    DeleteVm(String),
    RemoveDisk(String),
    RemoveNic(String),
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
    arch: String,
    isos: Vec<PathBuf>,
    iso: Option<PathBuf>,
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
        let arch = if cfg!(target_arch = "aarch64") {
            "aarch64"
        } else {
            "x86_64"
        };
        Self {
            arch: arch.into(),
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
    disks: Vec<DiskInfo>,
    nics: Vec<NetInfo>,
    disks_sel: Option<String>,
    nics_sel: Option<String>,
    disk_add_open: bool,
    disk_add_size: u32,
    disk_add_bus: usize,
    nic_model: usize,
    snaps: Vec<SnapInfo>,
    snap_arm: Option<SnapArm>,
    confirm: Option<ConfirmDialog>,
    stats: HashMap<String, VmStat>,
    form: CreateForm,
    host_ram_mb: u32,
    host_cpus: u32,
    kvm_ok: bool,
    status: Option<(String, StatusKind)>,
    lang: Lang,
    name_focus: FocusHandle,
    name_cursor: usize,
    name_manual: bool,
    name_fresh: bool,
    name_focus_pending: bool,
    ticks: u64,
    started: bool,
}

impl VmmApp {
    pub fn new(cx: &mut Context<Self>) -> Self {
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
            disks: Vec::new(),
            nics: Vec::new(),
            disks_sel: None,
            nics_sel: None,
            disk_add_open: false,
            disk_add_size: 20,
            disk_add_bus: 0,
            nic_model: 0,
            snaps: Vec::new(),
            snap_arm: None,
            confirm: None,
            stats: HashMap::new(),
            form,
            host_ram_mb,
            host_cpus,
            kvm_ok,
            status: None,
            lang: i18n::load_lang(),
            name_focus: cx.focus_handle(),
            name_cursor: 0,
            name_manual: false,
            name_fresh: false,
            name_focus_pending: false,
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

    fn t(&self, key: &str) -> &'static str {
        i18n::tr(self.lang, key)
    }

    fn tf(&self, key: &str, args: &[(&str, String)]) -> String {
        i18n::tf(self.lang, key, args)
    }

    fn set_lang(&mut self, lang: Lang, cx: &mut Context<Self>) {
        self.lang = lang;
        i18n::save_lang(lang);
        let msg = self.tf("status.lang_changed", &[("lang", lang.label().to_string())]);
        self.set_status(msg, StatusKind::Ok);
        cx.notify();
    }

    fn cycle_lang(&mut self, cx: &mut Context<Self>) {
        let next = i18n::next(self.lang);
        self.set_lang(next, cx);
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
        self.disks = self
            .selected
            .as_ref()
            .and_then(|name| self.backend.as_ref().and_then(|b| b.disks(name).ok()))
            .unwrap_or_default();
        self.nics = self
            .selected
            .as_ref()
            .and_then(|name| self.backend.as_ref().and_then(|b| b.nics(name).ok()))
            .unwrap_or_default();
        let disk_ok = self
            .disks_sel
            .as_ref()
            .map(|target| self.disks.iter().any(|disk| &disk.target == target))
            .unwrap_or(false);
        if !disk_ok {
            self.disks_sel = None;
        }
        let nic_ok = self
            .nics_sel
            .as_ref()
            .map(|mac| self.nics.iter().any(|nic| &nic.mac == mac))
            .unwrap_or(false);
        if !nic_ok {
            self.nics_sel = None;
        }
    }

    fn reload_snaps(&mut self) {
        self.snaps = self
            .selected
            .as_ref()
            .and_then(|name| self.backend.as_ref().and_then(|b| b.snapshots(name).ok()))
            .unwrap_or_default();
        self.disks = self
            .selected
            .as_ref()
            .and_then(|name| self.backend.as_ref().and_then(|b| b.disks(name).ok()))
            .unwrap_or_default();
        self.nics = self
            .selected
            .as_ref()
            .and_then(|name| self.backend.as_ref().and_then(|b| b.nics(name).ok()))
            .unwrap_or_default();
        let disk_ok = self
            .disks_sel
            .as_ref()
            .map(|target| self.disks.iter().any(|disk| &disk.target == target))
            .unwrap_or(false);
        if !disk_ok {
            self.disks_sel = None;
        }
        let nic_ok = self
            .nics_sel
            .as_ref()
            .map(|mac| self.nics.iter().any(|nic| &nic.mac == mac))
            .unwrap_or(false);
        if !nic_ok {
            self.nics_sel = None;
        }
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
                    let msg = self.tf("conn.connected", &[("uri", uri.to_string())]);
                    self.set_status(msg, StatusKind::Ok);
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
        if self.name_manual {
            return;
        }
        let profile = osdata::find(&self.form.profile_id);
        let base = osdata::auto_name(profile, &self.form.version);
        let base = if base.is_empty() {
            "vm".to_string()
        } else {
            base
        };
        self.form.name = self.unique_name(&base);
        self.name_cursor = self.form.name.len();
        self.name_fresh = false;
    }

    fn form_reset_name(&mut self, cx: &mut Context<Self>) {
        self.name_manual = false;
        self.name_fresh = false;
        self.form_sync_name();
        cx.notify();
    }

    fn focus_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.name_focus);
        self.name_cursor = self.form.name.len();
        self.name_fresh = !self.name_manual;
        cx.notify();
    }

    fn name_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        if matches!(key, "enter" | "escape") {
            window.blur();
            cx.notify();
            return;
        }
        if self.name_fresh && !matches!(key, "left" | "right" | "home" | "end") {
            self.form.name.clear();
            self.name_cursor = 0;
            self.name_fresh = false;
            self.name_manual = true;
        }
        match key {
            "backspace" => {
                if self.name_cursor > 0 {
                    let start = prev_boundary(&self.form.name, self.name_cursor);
                    self.form.name.replace_range(start..self.name_cursor, "");
                    self.name_cursor = start;
                }
            }
            "delete" => {
                if self.name_cursor < self.form.name.len() {
                    let end = next_boundary(&self.form.name, self.name_cursor);
                    self.form.name.replace_range(self.name_cursor..end, "");
                }
            }
            "left" => {
                self.name_cursor = prev_boundary(&self.form.name, self.name_cursor);
            }
            "right" => {
                self.name_cursor = next_boundary(&self.form.name, self.name_cursor);
            }
            "home" => self.name_cursor = 0,
            "end" => self.name_cursor = self.form.name.len(),
            "space" => {
                if !event.keystroke.modifiers.control && !event.keystroke.modifiers.platform {
                    self.form.name.insert(self.name_cursor, ' ');
                    self.name_cursor += 1;
                    self.name_manual = true;
                }
            }
            _ => {
                if !event.keystroke.modifiers.control && !event.keystroke.modifiers.platform {
                    if let Some(text) = &event.keystroke.key_char {
                        let mut changed = false;
                        for ch in text.chars() {
                            if ch.is_alphanumeric() || matches!(ch, '.' | '-' | '_' | ' ') {
                                self.form.name.insert(self.name_cursor, ch);
                                self.name_cursor += ch.len_utf8();
                                changed = true;
                            }
                        }
                        if changed {
                            self.name_manual = true;
                        }
                    }
                }
            }
        }
        cx.notify();
    }

    fn form_select_arch(&mut self, arch: &'static str, cx: &mut Context<Self>) {
        self.form.arch = arch.into();
        self.form.kvm = self.kvm_ok && backend::arch_native(arch);
        cx.notify();
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

    fn form_select_iso(&mut self, path: Option<PathBuf>, cx: &mut Context<Self>) {
        self.form.iso = path;
        if let Some(path) = self.form.iso.as_ref() {
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
        if let Some(path) = self.form.iso.as_ref() {
            if !path.exists() {
                self.form.iso = None;
            }
        }
        let key = if self.form.isos.is_empty() {
            "status.no_iso"
        } else {
            "status.iso_found"
        };
        let msg = self.tf(key, &[("n", self.form.isos.len().to_string())]);
        self.set_status(msg, StatusKind::Info);
        cx.notify();
    }

    fn form_pick_iso(&mut self, cx: &mut Context<Self>) {
        let title = self.t("cr.choose_iso").to_string();
        let start = backend::home_dir().join("VirtualMachines").join("iso");
        cx.spawn(async move |this, cx| {
            let picked = rfd::AsyncFileDialog::new()
                .add_filter("ISO", &["iso"])
                .set_title(title)
                .set_directory(start)
                .pick_file()
                .await;
            if let Some(file) = picked {
                let path = file.path().to_path_buf();
                let _ = this.update(cx, |this, cx| {
                    this.form_select_iso(Some(path), cx);
                });
            }
        })
        .detach();
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
        let name = self.form.name.trim().to_string();
        if name.is_empty() {
            self.set_status(self.t("cr.name_empty").to_string(), StatusKind::Error);
            cx.notify();
            return;
        }
        if self.vms.iter().any(|vm| vm.name == name) {
            let msg = self.tf("cr.name_taken", &[("name", name.clone())]);
            self.set_status(msg, StatusKind::Error);
            cx.notify();
            return;
        }
        let iso = self
            .form
            .iso
            .as_ref()
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_default();
        let profile = osdata::find(&self.form.profile_id);
        let request = backend::VmRequest {
            name: &name,
            profile,
            arch: &self.form.arch,
            vcpu: self.form.vcpu,
            ram_mb: self.form.ram,
            disk_gb: self.form.disk,
            iso: &iso,
            kvm: self.form.kvm,
        };
        let res = {
            let Some(b) = self.backend.as_ref() else {
                self.set_status(self.t("err.not_connected").to_string(), StatusKind::Error);
                cx.notify();
                return;
            };
            b.create_vm(&request)
        };
        match res {
            Ok(()) => {
                self.selected = Some(name.clone());
                self.screen = Screen::Vms;
                self.tab = Tab::Overview;
                self.refresh();
                self.load_details();
                let msg = self.tf("cr.created", &[("name", name.clone())]);
                self.set_status(msg, StatusKind::Ok);
            }
            Err(e) => {
                let msg = self.tf("cr.create_failed", &[("error", e.to_string())]);
                self.set_status(msg, StatusKind::Error);
            }
        }
        cx.notify();
    }

    fn vm_action(&mut self, action: VmAction, cx: &mut Context<Self>) {
        let Some(name) = self.selected.clone() else {
            return;
        };
        let res = {
            let Some(b) = self.backend.as_ref() else {
                self.set_status(self.t("err.not_connected").to_string(), StatusKind::Error);
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
                let key = match action {
                    VmAction::Start => "status.starting",
                    VmAction::Shutdown => "status.shutdown_requested",
                    VmAction::ForceOff => "status.powered_off",
                    VmAction::Reboot => "status.rebooting",
                    VmAction::Pause => "status.paused",
                    VmAction::Resume => "status.resumed",
                    VmAction::Console => "status.console",
                };
                let msg = if matches!(action, VmAction::Console) {
                    self.t(key).to_string()
                } else {
                    self.tf(key, &[("name", name.clone())])
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

    fn delete_selected(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = self.selected.clone() else {
            return;
        };
        let running = self
            .selected_vm()
            .map(|vm| vm.running() || vm.paused())
            .unwrap_or(false);
        if running {
            self.set_status(
                self.t("status.stop_before_delete").to_string(),
                StatusKind::Error,
            );
            cx.notify();
            return;
        }
        self.confirm = Some(ConfirmDialog {
            title: self.tf("status.delete_title", &[("name", name.clone())]),
            body: self.t("status.delete_body").to_string(),
            confirm: self.t("snap.delete").to_string(),
            action: ConfirmAction::DeleteVm(name),
            opened_at: Instant::now(),
        });
        cx.notify();
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
                let msg = self.tf("status.deleted", &[("name", name.to_string())]);
                self.set_status(msg, StatusKind::Ok);
            }
            Err(e) => {
                let msg = self.tf("status.delete_failed", &[("error", e.to_string())]);
                self.set_status(msg, StatusKind::Error);
            }
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
        let Some(hw) = self.hw.as_ref() else {
            return;
        };
        if !backend::arch_native(&hw.arch) {
            self.set_status(self.t("hw.kvm_foreign").to_string(), StatusKind::Info);
            cx.notify();
            return;
        }
        if let Some(hw) = self.hw.as_mut() {
            hw.kvm = !hw.kvm;
            cx.notify();
        }
    }

    fn hw_toggle_uefi(&mut self, cx: &mut Context<Self>) {
        let Some(hw) = self.hw.as_ref() else {
            return;
        };
        if hw.arch == "aarch64" {
            self.set_status(self.t("hw.uefi_arm").to_string(), StatusKind::Info);
            cx.notify();
            return;
        }
        if hw.arch == "i686" {
            self.set_status(self.t("hw.uefi_i686").to_string(), StatusKind::Info);
            cx.notify();
            return;
        }
        if let Some(hw) = self.hw.as_mut() {
            hw.uefi = !hw.uefi;
            cx.notify();
        }
    }

    fn hw_toggle_tpm(&mut self, cx: &mut Context<Self>) {
        let Some(hw) = self.hw.as_ref() else {
            return;
        };
        if hw.arch != "x86_64" {
            self.set_status(self.t("hw.tpm_arch").to_string(), StatusKind::Info);
            cx.notify();
            return;
        }
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
        let Some(hw) = self.hw.as_ref() else {
            return;
        };
        if hw.arch == "aarch64" {
            self.set_status(self.t("hw.video_arm").to_string(), StatusKind::Info);
            cx.notify();
            return;
        }
        if let Some(hw) = self.hw.as_mut() {
            hw.video = match hw.video.as_str() {
                "qxl" => "virtio".into(),
                "virtio" => "vga".into(),
                _ => "qxl".into(),
            };
            cx.notify();
        }
    }

    fn hw_cycle_machine(&mut self, cx: &mut Context<Self>) {
        let Some(hw) = self.hw.as_ref() else {
            return;
        };
        if hw.arch == "aarch64" || hw.arch == "i686" {
            cx.notify();
            return;
        }
        if let Some(hw) = self.hw.as_mut() {
            hw.machine = if hw.machine.contains("q35") {
                "pc".into()
            } else {
                "q35".into()
            };
            cx.notify();
        }
    }

    fn hw_set_arch(&mut self, arch: &'static str, cx: &mut Context<Self>) {
        let Some(vm) = self.selected_vm() else {
            return;
        };
        if self.hw.as_ref().map(|hw| hw.arch.as_str()) == Some(arch) {
            return;
        }
        if vm.running() || vm.paused() {
            self.set_status(self.t("hw.stop_for_arch").to_string(), StatusKind::Error);
            cx.notify();
            return;
        }
        if !backend::arch_available(arch) {
            let msg = self.tf("hw.arch_unavailable", &[("arch", arch.to_string())]);
            self.set_status(msg, StatusKind::Error);
            cx.notify();
            return;
        }
        let kvm_ok = self.kvm_ok;
        if let Some(hw) = self.hw.as_mut() {
            hw.arch = arch.into();
            match arch {
                "aarch64" => {
                    hw.machine = "virt".into();
                    hw.uefi = true;
                    hw.tpm = false;
                    hw.kvm = false;
                    hw.video = "virtio".into();
                }
                "i686" => {
                    hw.machine = "pc".into();
                    hw.uefi = false;
                    hw.tpm = false;
                    hw.kvm = kvm_ok;
                }
                _ => {
                    hw.machine = "q35".into();
                    hw.kvm = kvm_ok;
                }
            }
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
                    self.set_status(self.t("status.hw_deferred").to_string(), StatusKind::Info);
                } else {
                    self.set_status(self.t("status.hw_updated").to_string(), StatusKind::Ok);
                }
                self.refresh();
                self.load_details();
            }
            Err(e) => {
                let msg = self.tf("status.hw_failed", &[("error", e.to_string())]);
                self.set_status(msg, StatusKind::Error);
            }
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
                let msg = self.tf("snap.created", &[("name", snapname.clone())]);
                self.set_status(msg, StatusKind::Ok);
            }
            Err(e) => {
                let msg = self.tf("snap.failed", &[("error", e.to_string())]);
                self.set_status(msg, StatusKind::Error);
            }
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
                    SnapArm::Revert(snap) => self.tf("snap.reverted", &[("name", snap.clone())]),
                    SnapArm::Delete(snap) => self.tf("snap.deleted", &[("name", snap.clone())]),
                };
                self.set_status(msg, StatusKind::Ok);
                self.reload_snaps();
                self.refresh();
            }
            Err(e) => {
                let msg = self.tf("snap.op_failed", &[("error", e.to_string())]);
                self.set_status(msg, StatusKind::Error);
            }
        }
        cx.notify();
    }

    fn disk_toggle_add(&mut self, cx: &mut Context<Self>) {
        self.disk_add_open = !self.disk_add_open;
        cx.notify();
    }

    fn disk_size_step(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.disk_add_size = (self.disk_add_size as i32 + delta * 5).clamp(5, 2000) as u32;
        cx.notify();
    }

    fn disk_bus_cycle(&mut self, cx: &mut Context<Self>) {
        self.disk_add_bus = (self.disk_add_bus + 1) % DISK_BUSES.len();
        cx.notify();
    }

    fn disk_add(&mut self, cx: &mut Context<Self>) {
        let Some(name) = self.selected.clone() else {
            return;
        };
        let size = self.disk_add_size;
        let bus = DISK_BUSES[self.disk_add_bus];
        let res = {
            let Some(b) = self.backend.as_ref() else {
                self.set_status(self.t("err.not_connected").to_string(), StatusKind::Error);
                cx.notify();
                return;
            };
            b.add_disk(&name, size, bus)
        };
        match res {
            Ok(target) => {
                let msg = self.tf(
                    "disk.added",
                    &[("target", target.clone()), ("size", size.to_string())],
                );
                self.set_status(msg, StatusKind::Ok);
                self.disk_add_open = false;
                self.load_details();
            }
            Err(e) => {
                let msg = self.tf("disk.add_failed", &[("error", e.to_string())]);
                self.set_status(msg, StatusKind::Error);
            }
        }
        cx.notify();
    }

    fn disk_remove(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.disks_sel.clone() else {
            self.set_status(self.t("disk.select_hint").to_string(), StatusKind::Info);
            cx.notify();
            return;
        };
        self.confirm = Some(ConfirmDialog {
            title: self.tf("disk.remove_title", &[("target", target.clone())]),
            body: self.t("disk.remove_body").to_string(),
            confirm: self.t("snap.delete").to_string(),
            action: ConfirmAction::RemoveDisk(target),
            opened_at: Instant::now(),
        });
        cx.notify();
    }

    fn disk_remove_now(&mut self, target: &str, cx: &mut Context<Self>) {
        let Some(name) = self.selected.clone() else {
            return;
        };
        let res = {
            let Some(b) = self.backend.as_ref() else {
                return;
            };
            b.remove_disk(&name, target)
        };
        match res {
            Ok(()) => {
                let msg = self.tf("disk.removed", &[("target", target.to_string())]);
                self.set_status(msg, StatusKind::Ok);
                self.load_details();
            }
            Err(e) => {
                let msg = self.tf("disk.remove_failed", &[("error", e.to_string())]);
                self.set_status(msg, StatusKind::Error);
            }
        }
        cx.notify();
    }

    fn iso_attach_pick(&mut self, cx: &mut Context<Self>) {
        let title = self.t("disk.attach_iso").to_string();
        let start = backend::home_dir().join("VirtualMachines").join("iso");
        cx.spawn(async move |this, cx| {
            let picked = rfd::AsyncFileDialog::new()
                .add_filter("ISO", &["iso"])
                .set_title(title)
                .set_directory(start)
                .pick_file()
                .await;
            if let Some(file) = picked {
                let path = file.path().to_path_buf();
                let _ = this.update(cx, |this, cx| this.iso_attach_now(path, cx));
            }
        })
        .detach();
    }

    fn iso_attach_now(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let Some(name) = self.selected.clone() else {
            return;
        };
        let res = {
            let Some(b) = self.backend.as_ref() else {
                return;
            };
            b.attach_iso(&name, Some(&path.to_string_lossy()))
        };
        match res {
            Ok(()) => {
                self.set_status(self.t("disk.iso_attached").to_string(), StatusKind::Ok);
                self.load_details();
            }
            Err(e) => {
                let msg = self.tf("disk.iso_failed", &[("error", e.to_string())]);
                self.set_status(msg, StatusKind::Error);
            }
        }
        cx.notify();
    }

    fn iso_eject(&mut self, cx: &mut Context<Self>) {
        let Some(name) = self.selected.clone() else {
            return;
        };
        let res = {
            let Some(b) = self.backend.as_ref() else {
                return;
            };
            b.attach_iso(&name, None)
        };
        match res {
            Ok(()) => {
                self.set_status(self.t("disk.iso_ejected").to_string(), StatusKind::Ok);
                self.load_details();
            }
            Err(e) => {
                let msg = self.tf("disk.iso_failed", &[("error", e.to_string())]);
                self.set_status(msg, StatusKind::Error);
            }
        }
        cx.notify();
    }

    fn nic_model_cycle(&mut self, cx: &mut Context<Self>) {
        self.nic_model = (self.nic_model + 1) % NIC_MODELS.len();
        cx.notify();
    }

    fn nic_add(&mut self, cx: &mut Context<Self>) {
        let Some(name) = self.selected.clone() else {
            return;
        };
        let model = NIC_MODELS[self.nic_model];
        let res = {
            let Some(b) = self.backend.as_ref() else {
                self.set_status(self.t("err.not_connected").to_string(), StatusKind::Error);
                cx.notify();
                return;
            };
            b.add_nic(&name, model)
        };
        match res {
            Ok(mac) => {
                let msg = self.tf("net.added", &[("mac", mac)]);
                self.set_status(msg, StatusKind::Ok);
                self.load_details();
            }
            Err(e) => {
                let msg = self.tf("net.add_failed", &[("error", e.to_string())]);
                self.set_status(msg, StatusKind::Error);
            }
        }
        cx.notify();
    }

    fn nic_remove(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(mac) = self.nics_sel.clone() else {
            self.set_status(self.t("net.select_hint").to_string(), StatusKind::Info);
            cx.notify();
            return;
        };
        self.confirm = Some(ConfirmDialog {
            title: self.tf("net.remove_title", &[("mac", mac.clone())]),
            body: self.t("net.remove_body").to_string(),
            confirm: self.t("snap.delete").to_string(),
            action: ConfirmAction::RemoveNic(mac),
            opened_at: Instant::now(),
        });
        cx.notify();
    }

    fn nic_remove_now(&mut self, mac: &str, cx: &mut Context<Self>) {
        let Some(name) = self.selected.clone() else {
            return;
        };
        let res = {
            let Some(b) = self.backend.as_ref() else {
                return;
            };
            b.remove_nic(&name, mac)
        };
        match res {
            Ok(()) => {
                let msg = self.tf("net.removed", &[("mac", mac.to_string())]);
                self.set_status(msg, StatusKind::Ok);
                self.load_details();
            }
            Err(e) => {
                let msg = self.tf("net.remove_failed", &[("error", e.to_string())]);
                self.set_status(msg, StatusKind::Error);
            }
        }
        cx.notify();
    }

    fn nav(&mut self, screen: Screen, cx: &mut Context<Self>) {
        if screen == Screen::Create {
            self.form_sync_name();
            self.name_focus_pending = true;
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
        let title = match self.screen {
            Screen::Vms => self.t("title.vms"),
            Screen::Create => self.t("title.create"),
            Screen::Storage => self.t("title.storage"),
            Screen::Networks => self.t("title.networks"),
            Screen::Ssh => self.t("title.ssh"),
            Screen::Settings => self.t("title.settings"),
        };
        let subtitle = match self.screen {
            Screen::Vms => self.tf(
                "subtitle.vms",
                &[
                    ("n", self.vms.len().to_string()),
                    ("running", running.to_string()),
                ],
            ),
            Screen::Create => self.t("subtitle.create").to_string(),
            Screen::Storage => self.t("subtitle.storage").to_string(),
            Screen::Networks => self.t("subtitle.networks").to_string(),
            Screen::Ssh => self.t("subtitle.ssh").to_string(),
            Screen::Settings => self.t("subtitle.settings").to_string(),
        };
        (title, subtitle)
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> Div {
        let conn = match &self.backend {
            Some(b) => {
                let label = if b.is_session() { "session" } else { "system" };
                format!("libvirt · {label}")
            }
            None => self.t("conn.offline").to_string(),
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
                            .w(px(34.))
                            .h(px(34.))
                            .rounded(px(8.))
                            .bg(col(ACCENT))
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap(px(2.))
                            .child(
                                div()
                                    .w(px(20.))
                                    .h(px(13.))
                                    .rounded(px(3.))
                                    .bg(col(BG))
                                    .flex()
                                    .items_center()
                                    .gap(px(2.))
                                    .px(px(3.))
                                    .child(dot(col(ACCENT), 3.0))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(1.5))
                                            .child(
                                                div()
                                                    .w(px(8.))
                                                    .h(px(1.5))
                                                    .rounded(px(0.75))
                                                    .bg(col(ACCENT)),
                                            )
                                            .child(
                                                div()
                                                    .w(px(6.))
                                                    .h(px(1.5))
                                                    .rounded(px(0.75))
                                                    .bg(alpha(col(ACCENT), 0.6)),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .w(px(16.))
                                    .h(px(3.))
                                    .rounded(px(1.5))
                                    .bg(alpha(col(BG), 0.55)),
                            ),
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
                                    .child(self.t("app.tagline")),
                            ),
                    ),
            )
            .child(self.nav_item("nav_vms", self.t("nav.vms"), "▣", Screen::Vms, cx))
            .child(self.nav_item("nav_create", self.t("nav.new"), "＋", Screen::Create, cx))
            .child(div().h(px(10.)))
            .child(self.nav_item(
                "nav_storage",
                self.t("nav.storage"),
                "▤",
                Screen::Storage,
                cx,
            ))
            .child(self.nav_item(
                "nav_networks",
                self.t("nav.networks"),
                "⇄",
                Screen::Networks,
                cx,
            ))
            .child(self.nav_item("nav_ssh", self.t("nav.ssh"), "⌗", Screen::Ssh, cx))
            .child(self.nav_item(
                "nav_settings",
                self.t("nav.settings"),
                "⚙",
                Screen::Settings,
                cx,
            ))
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
                            .child(self.t("hypervisor")),
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
                                    self.t("kvm.available")
                                } else {
                                    self.t("kvm.unavailable")
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
            .child(div().flex_1().min_w(px(0.)).truncate().child(label))
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
                                    self.t("tb.connected")
                                } else {
                                    self.t("tb.offline")
                                },
                            )),
                    )
                    .child(
                        button("tb_lang", i18n::short(self.lang), BtnKind::Normal).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| this.cycle_lang(cx)),
                        ),
                    )
                    .child(
                        button("tb_refresh", self.t("tb.refresh"), BtnKind::Normal).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.refresh();
                                this.set_status(
                                    this.t("status.refreshed").to_string(),
                                    StatusKind::Info,
                                );
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        button(
                            "tb_new",
                            &format!("＋ {}", self.t("tb.new")),
                            BtnKind::Accent,
                        )
                        .on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| this.nav(Screen::Create, cx)),
                        ),
                    ),
            )
    }

    fn body(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.backend.is_none() {
            return self.connection_error(cx).into_any_element();
        }
        match self.screen {
            Screen::Vms => self.vms_screen(cx).into_any_element(),
            Screen::Create => self.create_screen(window, cx).into_any_element(),
            Screen::Storage => self
                .placeholder(
                    "▤",
                    self.t("ph.storage_title"),
                    self.t("ph.storage_body"),
                    cx,
                )
                .into_any_element(),
            Screen::Networks => self
                .placeholder(
                    "⇄",
                    self.t("ph.networks_title"),
                    self.t("ph.networks_body"),
                    cx,
                )
                .into_any_element(),
            Screen::Ssh => self
                .placeholder("⌗", self.t("ph.ssh_title"), self.t("ph.ssh_body"), cx)
                .into_any_element(),
            Screen::Settings => self.settings_screen(cx).into_any_element(),
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
                            .child(self.t("conn.title")),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(col(MUTED))
                            .child(self.t("conn.body")),
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
                    .child(div().flex().gap(px(8.)).child(
                        button("retry_conn", self.t("conn.retry"), BtnKind::Accent).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| this.reconnect(cx)),
                        ),
                    )),
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
                                div().text_size(px(13.)).text_color(col(MUTED)).child(
                                    self.tf(
                                        "list.summary",
                                        &[
                                            ("n", self.vms.len().to_string()),
                                            (
                                                "running",
                                                self.vms
                                                    .iter()
                                                    .filter(|v| v.running())
                                                    .count()
                                                    .to_string(),
                                            ),
                                        ],
                                    ),
                                ),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(col(MUTED))
                                    .child(self.t("list.hint")),
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
                            .child(self.state_badge(vm)),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(col(MUTED))
                            .child(format!(
                                "{} · {}",
                                profile.name,
                                self.tf(
                                    "vm.spec",
                                    &[
                                        ("vcpu", vm.vcpus.to_string()),
                                        ("ram", fmt_mb(vm.maxmem_mb)),
                                        ("disk", fmt_disk(vm.disk_gb)),
                                    ],
                                )
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
            .w(px(410.))
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
                    .child(self.tab_button(Tab::Overview, self.t("tab.overview"), cx))
                    .child(self.tab_button(Tab::Hardware, self.t("tab.hardware"), cx))
                    .child(self.tab_button(Tab::Disks, self.t("tab.disks"), cx))
                    .child(self.tab_button(Tab::Network, self.t("tab.network"), cx))
                    .child(self.tab_button(Tab::Snapshots, self.t("tab.snapshots"), cx)),
            )
            .child(
                div()
                    .id("details_scroll")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .p(px(16.))
                    .child(div().w_full().child(match self.tab {
                        Tab::Overview => self.overview_tab(cx),
                        Tab::Hardware => self.hardware_tab(cx),
                        Tab::Disks => self.disks_tab(cx),
                        Tab::Network => self.network_tab(cx),
                        Tab::Snapshots => self.snapshots_tab(cx),
                    })),
            )
    }

    fn tab_button(&self, tab: Tab, label: &'static str, cx: &mut Context<Self>) -> Stateful<Div> {
        let active = self.tab == tab;
        div()
            .id(match tab {
                Tab::Overview => "tab_overview",
                Tab::Hardware => "tab_hardware",
                Tab::Disks => "tab_disks",
                Tab::Network => "tab_network",
                Tab::Snapshots => "tab_snapshots",
            })
            .flex_1()
            .flex()
            .justify_center()
            .py(px(11.))
            .border_b_1()
            .border_color(if active { col(ACCENT) } else { col(BORDER) })
            .text_size(px(11.))
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
            format!(
                "{} ({})",
                vm.vcpus,
                self.tf("ov.now", &[("value", vm.live_vcpus.to_string())])
            )
        } else {
            vm.vcpus.to_string()
        };
        let ram_value = if vm.running() && vm.live_maxmem_mb != vm.maxmem_mb {
            format!(
                "{} ({})",
                fmt_mb(vm.maxmem_mb),
                self.tf("ov.now", &[("value", fmt_mb(vm.live_maxmem_mb))])
            )
        } else {
            fmt_mb(vm.maxmem_mb)
        };
        let machine = hw.map(|h| h.machine.clone()).unwrap_or_else(|| "—".into());
        let uefi = hw
            .map(|h| if h.uefi { "UEFI" } else { "BIOS" })
            .unwrap_or("—");
        let tpm = hw
            .map(|h| {
                if h.tpm {
                    self.t("ov.yes")
                } else {
                    self.t("ov.no")
                }
            })
            .unwrap_or("—");
        let video = hw.map(|h| h.video.clone()).unwrap_or_else(|| "—".into());

        let mut actions: Vec<Stateful<Div>> = Vec::new();
        if vm.running() {
            actions.push(self.action_button(
                "act_shutdown",
                self.t("act.shutdown"),
                BtnKind::Normal,
                VmAction::Shutdown,
                cx,
            ));
            actions.push(self.action_button(
                "act_force",
                self.t("act.forceoff"),
                BtnKind::Danger,
                VmAction::ForceOff,
                cx,
            ));
            actions.push(self.action_button(
                "act_reboot",
                self.t("act.reboot"),
                BtnKind::Normal,
                VmAction::Reboot,
                cx,
            ));
            actions.push(self.action_button(
                "act_pause",
                self.t("act.pause"),
                BtnKind::Normal,
                VmAction::Pause,
                cx,
            ));
        } else if vm.paused() {
            actions.push(self.action_button(
                "act_resume2",
                self.t("act.resume"),
                BtnKind::Accent,
                VmAction::Resume,
                cx,
            ));
            actions.push(self.action_button(
                "act_force2",
                self.t("act.forceoff"),
                BtnKind::Danger,
                VmAction::ForceOff,
                cx,
            ));
        } else {
            actions.push(self.action_button(
                "act_start",
                self.t("act.start"),
                BtnKind::Accent,
                VmAction::Start,
                cx,
            ));
        }
        actions.push(self.action_button(
            "act_console",
            self.t("act.console"),
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
                                    .child(self.state_badge(vm))
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
                    .child(self.info_row(self.t("ov.os"), profile.name))
                    .child(self.info_row(self.t("ov.state"), self.state_name(vm)))
                    .child(self.info_row(self.t("ov.vcpu"), vcpu_value))
                    .child(self.info_row(self.t("ov.memory"), ram_value))
                    .child(self.info_row(self.t("ov.disk"), fmt_disk(vm.disk_gb)))
                    .child(
                        self.info_row(
                            self.t("ov.arch"),
                            hw.map(|value| value.arch.clone())
                                .unwrap_or_else(|| "x86_64".into()),
                        ),
                    )
                    .child(self.info_row(self.t("ov.machine"), machine))
                    .child(self.info_row(self.t("ov.firmware"), uefi))
                    .child(self.info_row(self.t("ov.tpm"), tpm))
                    .child(self.info_row(self.t("ov.video"), video))
                    .child(self.info_row(
                        self.t("ov.autostart"),
                        if vm.autostart {
                            self.t("ov.on")
                        } else {
                            self.t("ov.off")
                        },
                    ))
                    .child(self.info_row(self.t("ov.console"), "SPICE")),
            )
            .child(
                div()
                    .text_size(px(10.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(col(MUTED))
                    .child(self.t("ov.power")),
            )
            .child(div().flex().flex_wrap().gap(px(8.)).children(actions))
            .child(div().h(px(4.)))
            .child(
                div()
                    .text_size(px(10.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(col(MUTED))
                    .child(self.t("ov.danger")),
            )
            .child(
                div().w_full().flex().child(
                    button("act_delete", self.t("act.delete_vm"), BtnKind::Danger)
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
            .min_w(px(108.))
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

    fn disks_tab(&self, cx: &mut Context<Self>) -> Div {
        let Some(vm) = self.selected_vm() else {
            return self.no_selection();
        };
        let mut rows: Vec<Stateful<Div>> = Vec::new();
        for disk in &self.disks {
            let selected = self.disks_sel.as_deref() == Some(disk.target.as_str());
            let target = disk.target.clone();
            let file_name = std::path::Path::new(&disk.file)
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| disk.file.clone());
            let size = if disk.device == "cdrom" {
                self.t("disk.media").to_string()
            } else {
                format!("{} GiB", disk.size_gb)
            };
            rows.push(
                div()
                    .id(SharedString::from(format!("disk-{}", disk.target)))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(10.))
                    .py(px(7.))
                    .rounded(px(8.))
                    .bg(col(if selected { CARD2 } else { CARD }))
                    .border_1()
                    .border_color(col(if selected { ACCENT } else { BORDER }))
                    .cursor_pointer()
                    .hover(|this| this.border_color(col(ACCENT_D)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.disks_sel = Some(target.clone());
                        cx.notify();
                    }))
                    .child(
                        div()
                            .w(px(46.))
                            .text_size(px(11.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(col(ACCENT))
                            .child(disk.target.clone()),
                    )
                    .child(
                        div()
                            .w(px(44.))
                            .text_size(px(10.))
                            .text_color(col(MUTED))
                            .child(disk.bus.clone()),
                    )
                    .child(div().w(px(58.)).text_size(px(10.)).child(size))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_size(px(10.))
                            .text_color(col(MUTED))
                            .child(file_name),
                    ),
            );
        }
        let selected_hint = self
            .disks_sel
            .as_ref()
            .map(|target| self.tf("disk.selected", &[("target", target.clone())]))
            .unwrap_or_else(|| self.t("disk.select_hint").to_string());
        let mut add_form: Vec<Div> = Vec::new();
        if self.disk_add_open {
            add_form.push(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_y(px(6.))
                    .p(px(10.))
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
                                    .text_size(px(10.))
                                    .text_color(col(MUTED))
                                    .child(self.t("disk.size_label")),
                            )
                            .child(step_button("disk_size_minus", "−").on_click(cx.listener(
                                |this, _: &ClickEvent, _, cx| this.disk_size_step(-1, cx),
                            )))
                            .child(
                                div()
                                    .w(px(62.))
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(format!("{} GiB", self.disk_add_size)),
                            )
                            .child(step_button("disk_size_plus", "＋").on_click(cx.listener(
                                |this, _: &ClickEvent, _, cx| this.disk_size_step(1, cx),
                            ))),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                step_button(
                                    "disk_bus",
                                    &format!(
                                        "{} {}",
                                        self.t("disk.bus_label"),
                                        DISK_BUSES[self.disk_add_bus]
                                    ),
                                )
                                .w(px(96.))
                                .on_click(cx.listener(
                                    |this, _: &ClickEvent, _, cx| this.disk_bus_cycle(cx),
                                )),
                            )
                            .child(
                                button("disk_create", self.t("disk.create"), BtnKind::Accent)
                                    .on_click(
                                        cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.disk_add(cx)
                                        }),
                                    ),
                            )
                            .child(
                                button("disk_cancel", self.t("disk.cancel"), BtnKind::Normal)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.disk_toggle_add(cx)
                                    })),
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
                                    .child(self.t("disk.title")),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(col(MUTED))
                                    .child(format!("{} · {}", self.disks.len(), vm.name)),
                            ),
                    )
                    .child(
                        button("disk_add", self.t("disk.add"), BtnKind::Accent).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| this.disk_toggle_add(cx)),
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .px(px(10.))
                    .text_size(px(9.))
                    .text_color(col(MUTED))
                    .child(div().w(px(46.)).child(self.t("disk.device")))
                    .child(div().w(px(44.)).child(self.t("disk.bus")))
                    .child(div().w(px(58.)).child(self.t("disk.size")))
                    .child(div().flex_1().child(self.t("disk.file"))),
            )
            .child(div().flex().flex_col().gap(px(6.)).children(rows))
            .child(if self.disks.is_empty() {
                div()
                    .py(px(20.))
                    .text_size(px(11.))
                    .text_color(col(MUTED))
                    .child(self.t("disk.none"))
            } else {
                div()
            })
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(col(MUTED))
                    .child(selected_hint),
            )
            .children(add_form)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(6.))
                    .child(
                        button(
                            "disk_attach_iso",
                            self.t("disk.attach_iso"),
                            BtnKind::Normal,
                        )
                        .on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| this.iso_attach_pick(cx)),
                        ),
                    )
                    .child(
                        button("disk_eject_iso", self.t("disk.eject_iso"), BtnKind::Normal)
                            .on_click(
                                cx.listener(|this, _: &ClickEvent, _, cx| this.iso_eject(cx)),
                            ),
                    )
                    .child(
                        button("disk_remove", self.t("disk.remove"), BtnKind::Danger).on_click(
                            cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.disk_remove(window, cx)
                            }),
                        ),
                    ),
            )
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(col(if vm.running() { AMBER } else { MUTED }))
                    .child(self.t("disk.running_note")),
            )
    }

    fn network_tab(&self, cx: &mut Context<Self>) -> Div {
        let Some(vm) = self.selected_vm() else {
            return self.no_selection();
        };
        let mut rows: Vec<Stateful<Div>> = Vec::new();
        for nic in &self.nics {
            let selected = self.nics_sel.as_deref() == Some(nic.mac.as_str());
            let mac = nic.mac.clone();
            let type_label = match nic.iface_type.as_str() {
                "user" => self.t("net.user").to_string(),
                "bridge" => self.t("net.bridge").to_string(),
                _ => self.t("net.network").to_string(),
            };
            let source = if nic.source.is_empty() {
                type_label
            } else {
                format!("{type_label} · {}", nic.source)
            };
            rows.push(
                div()
                    .id(SharedString::from(format!("nic-{}", nic.mac)))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(10.))
                    .py(px(7.))
                    .rounded(px(8.))
                    .bg(col(if selected { CARD2 } else { CARD }))
                    .border_1()
                    .border_color(col(if selected { ACCENT } else { BORDER }))
                    .cursor_pointer()
                    .hover(|this| this.border_color(col(ACCENT_D)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.nics_sel = Some(mac.clone());
                        cx.notify();
                    }))
                    .child(
                        div()
                            .w(px(130.))
                            .text_size(px(10.))
                            .text_color(col(ACCENT))
                            .child(nic.mac.clone()),
                    )
                    .child(div().w(px(62.)).text_size(px(10.)).child(nic.model.clone()))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_size(px(10.))
                            .text_color(col(MUTED))
                            .child(source),
                    ),
            );
        }
        let selected_hint = self
            .nics_sel
            .as_ref()
            .map(|mac| {
                let value = self
                    .nics
                    .iter()
                    .find(|nic| &nic.mac == mac)
                    .map(|nic| nic.mac.clone())
                    .unwrap_or_default();
                self.tf("disk.selected", &[("target", value)])
            })
            .unwrap_or_else(|| self.t("net.select_hint").to_string());
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
                                    .child(self.t("net.title")),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(col(MUTED))
                                    .child(format!("{} · {}", self.nics.len(), vm.name)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                step_button(
                                    "nic_model",
                                    &format!(
                                        "{} {}",
                                        self.t("net.model_label"),
                                        NIC_MODELS[self.nic_model]
                                    ),
                                )
                                .w(px(104.))
                                .on_click(cx.listener(
                                    |this, _: &ClickEvent, _, cx| this.nic_model_cycle(cx),
                                )),
                            )
                            .child(
                                button("nic_add", self.t("net.add"), BtnKind::Accent).on_click(
                                    cx.listener(|this, _: &ClickEvent, _, cx| this.nic_add(cx)),
                                ),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .px(px(10.))
                    .text_size(px(9.))
                    .text_color(col(MUTED))
                    .child(div().w(px(130.)).child(self.t("net.mac")))
                    .child(div().w(px(62.)).child(self.t("net.model")))
                    .child(div().flex_1().child(self.t("net.source"))),
            )
            .child(div().flex().flex_col().gap(px(6.)).children(rows))
            .child(if self.nics.is_empty() {
                div()
                    .py(px(20.))
                    .text_size(px(11.))
                    .text_color(col(MUTED))
                    .child(self.t("net.none"))
            } else {
                div()
            })
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(col(MUTED))
                    .child(selected_hint),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(col(MUTED))
                            .child(self.t("net.add_hint")),
                    )
                    .child(
                        button("nic_remove", self.t("net.remove"), BtnKind::Danger).on_click(
                            cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.nic_remove(window, cx)
                            }),
                        ),
                    ),
            )
    }

    fn settings_screen(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let mut rows: Vec<Stateful<Div>> = Vec::new();
        for lang in Lang::ALL {
            let selected = self.lang == lang;
            rows.push(
                div()
                    .id(SharedString::from(format!("lang-{}", lang.code())))
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(12.))
                    .h(px(40.))
                    .rounded(px(8.))
                    .bg(col(if selected { CARD2 } else { CARD }))
                    .border_1()
                    .border_color(col(if selected { ACCENT } else { BORDER }))
                    .cursor_pointer()
                    .hover(|this| this.border_color(col(ACCENT_D)))
                    .on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| this.set_lang(lang, cx)),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(lang.label()),
                    )
                    .child(if selected {
                        div().w(px(8.)).h(px(8.)).rounded(px(4.)).bg(col(ACCENT))
                    } else {
                        div()
                    }),
            );
        }
        div()
            .flex_1()
            .min_h(px(0.))
            .id("settings_scroll")
            .overflow_y_scroll()
            .p(px(20.))
            .child(
                div()
                    .w_full()
                    .max_w(px(720.))
                    .mx_auto()
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(FontWeight::BOLD)
                            .child(self.t("title.settings")),
                    )
                    .child(
                        div()
                            .p(px(14.))
                            .rounded(px(10.))
                            .bg(col(PANEL))
                            .border_1()
                            .border_color(col(BORDER))
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.t("set.language")),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(col(MUTED))
                                    .child(self.t("set.language_hint")),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(col(ACCENT))
                                    .child(self.tf(
                                        "settings.current",
                                        &[("lang", self.lang.label().to_string())],
                                    )),
                            )
                            .child(div().flex().flex_col().gap(px(6.)).children(rows)),
                    )
                    .child(
                        div()
                            .p(px(14.))
                            .rounded(px(10.))
                            .bg(col(PANEL))
                            .border_1()
                            .border_color(col(BORDER))
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.t("set.about")),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(col(MUTED))
                                    .child(self.t("set.about_text")),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(col(MUTED))
                                    .child(format!(
                                        "{} {} · {} GPL-3.0",
                                        self.t("set.version"),
                                        env!("CARGO_PKG_VERSION"),
                                        self.t("set.license")
                                    )),
                            ),
                    ),
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
                                    if revert_armed {
                                        self.t("snap.confirm")
                                    } else {
                                        self.t("snap.revert")
                                    },
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
                                    if delete_armed {
                                        self.t("snap.confirm")
                                    } else {
                                        self.t("snap.delete")
                                    },
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
                                    .child(self.t("snap.title")),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(col(MUTED))
                                    .child(format!(
                                        "{} snapshot(s) · {}",
                                        self.snaps.len(),
                                        vm.name
                                    )),
                            ),
                    )
                    .child(
                        button("snap_create", self.t("snap.create"), BtnKind::Accent).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| this.snap_create(cx)),
                        ),
                    ),
            )
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(col(if vm.running() { AMBER } else { MUTED }))
                    .child(if vm.running() {
                        self.t("snap.warn_running")
                    } else {
                        self.t("snap.note")
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
                            .child(self.t("snap.empty")),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(col(MUTED))
                            .child(self.t("snap.empty_hint")),
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
                    .child(self.t("det.no_vm")),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(col(MUTED))
                    .child(self.t("det.select_vm")),
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
                .child(self.t("hw.unavailable"));
        };
        let mut arch_buttons: Vec<Stateful<Div>> = Vec::new();
        for arch in backend::ARCHES {
            if !backend::arch_available(arch) && hw.arch != arch {
                continue;
            }
            let id = match arch {
                "i686" => "hw_arch_i686",
                "aarch64" => "hw_arch_aarch64",
                _ => "hw_arch_x86_64",
            };
            let label = match arch {
                "i686" => "i686",
                "aarch64" => "ARM64",
                _ => "x86_64",
            };
            let selected = hw.arch == arch;
            arch_buttons.push(
                button(
                    id,
                    label,
                    if selected {
                        BtnKind::Accent
                    } else {
                        BtnKind::Normal
                    },
                )
                .h(px(26.))
                .on_click(
                    cx.listener(move |this, _: &ClickEvent, _, cx| this.hw_set_arch(arch, cx)),
                ),
            );
        }
        let arch_native = backend::arch_native(&hw.arch);
        let machine_value = if hw.arch == "aarch64" {
            "virt".to_string()
        } else if hw.arch == "i686" || !hw.machine.contains("q35") {
            "pc / i440fx".to_string()
        } else {
            "q35".to_string()
        };
        let video_value = if hw.arch == "aarch64" {
            "virtio".to_string()
        } else {
            hw.video.clone()
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
                            .child(self.t("tab.hardware")),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(col(MUTED))
                            .child(self.tf("hw.subtitle", &[("vm", vm.name.clone())])),
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
                        self.t("hw.vcpu"),
                        hw.vcpu.to_string(),
                        &self.tf("hw.range_vcpu", &[("max", self.host_cpus.to_string())]),
                        "hw_vcpu_minus",
                        "hw_vcpu_plus",
                        HwField::Vcpu,
                        cx,
                    ))
                    .child(self.hw_stepper(
                        self.t("hw.memory"),
                        fmt_mb(hw.ram),
                        &self.tf("hw.range_ram", &[("max", fmt_mb(self.host_ram_mb))]),
                        "hw_ram_minus",
                        "hw_ram_plus",
                        HwField::Ram,
                        cx,
                    ))
                    .child(
                        div()
                            .py(px(12.))
                            .border_b_1()
                            .border_color(col(BORDER))
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(self.t("hw.arch")),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(col(MUTED))
                                            .child(self.t("hw.arch_hint")),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .children(arch_buttons),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .py(px(12.))
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
                                            .child(self.t("hw.machine")),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(col(MUTED))
                                            .child(self.t("hw.machine_hint")),
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
                                            .child(machine_value),
                                    )
                                    .child(if hw.arch == "x86_64" {
                                        button("hw_machine", self.t("hw.change"), BtnKind::Normal)
                                            .h(px(28.))
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.hw_cycle_machine(cx)
                                            }))
                                            .into_any_element()
                                    } else {
                                        div()
                                            .text_size(px(9.))
                                            .text_color(col(MUTED))
                                            .child(self.t("hw.fixed"))
                                            .into_any_element()
                                    }),
                            ),
                    )
                    .child(
                        toggle_row(
                            self.t("hw.kvm"),
                            if !arch_native {
                                self.t("hw.kvm_foreign")
                            } else if self.kvm_ok {
                                self.t("hw.kvm_hint")
                            } else {
                                self.t("hw.kvm_missing")
                            },
                            hw.kvm,
                        )
                        .id("hw_kvm")
                        .on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| this.hw_toggle_kvm(cx)),
                        ),
                    )
                    .child(
                        toggle_row(
                            self.t("hw.uefi"),
                            if hw.arch == "aarch64" {
                                self.t("hw.uefi_arm")
                            } else if hw.arch == "i686" {
                                self.t("hw.uefi_i686")
                            } else {
                                self.t("hw.uefi_hint")
                            },
                            hw.uefi,
                        )
                        .id("hw_uefi")
                        .on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| this.hw_toggle_uefi(cx)),
                        ),
                    )
                    .child(
                        toggle_row(
                            self.t("hw.tpm"),
                            if hw.arch == "x86_64" {
                                self.t("hw.tpm_hint")
                            } else {
                                self.t("hw.tpm_arch")
                            },
                            hw.tpm,
                        )
                        .id("hw_tpm")
                        .on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| this.hw_toggle_tpm(cx)),
                        ),
                    )
                    .child(
                        toggle_row(
                            self.t("hw.autostart"),
                            self.t("hw.autostart_hint"),
                            hw.autostart,
                        )
                        .id("hw_autostart")
                        .on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| this.hw_toggle_autostart(cx)),
                        ),
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
                                            .child(self.t("hw.video")),
                                    )
                                    .child(div().text_size(px(10.)).text_color(col(MUTED)).child(
                                        if hw.arch == "aarch64" {
                                            self.t("hw.video_arm")
                                        } else {
                                            self.t("hw.video_hint")
                                        },
                                    )),
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
                                            .child(video_value),
                                    )
                                    .child(
                                        step_button("hw_video", self.t("hw.change"))
                                            .w(px(64.))
                                            .on_click(cx.listener(
                                                |this, _: &ClickEvent, _, cx| {
                                                    this.hw_cycle_video(cx)
                                                },
                                            )),
                                    ),
                            ),
                    ),
            )
            .child(
                div().w_full().flex().child(
                    button("hw_apply", self.t("hw.apply"), BtnKind::Accent)
                        .h(px(34.))
                        .flex_1()
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.hw_apply(cx))),
                ),
            )
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(col(MUTED))
                    .child(self.t("hw.note")),
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

    fn create_screen(&self, window: &mut Window, cx: &mut Context<Self>) -> Stateful<Div> {
        let mut iso_rows: Vec<Stateful<Div>> = Vec::new();
        iso_rows.push(self.iso_row(
            None,
            self.t("cr.no_media"),
            self.t("cr.no_media_hint"),
            "other",
            cx,
        ));
        for path in &self.form.isos {
            let file = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string());
            let profile = osdata::detect_from_iso(&path.to_string_lossy()).0;
            iso_rows.push(self.iso_row(
                Some(path.clone()),
                &file,
                &format!("{} · ~/VirtualMachines/iso", profile.name),
                profile.icon,
                cx,
            ));
        }
        if let Some(custom) = self.form.iso.as_ref() {
            if !self.form.isos.iter().any(|path| path == custom) {
                let name = custom
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| custom.to_string_lossy().to_string());
                let profile = osdata::detect_from_iso(&custom.to_string_lossy()).0;
                let subtitle = self.tf("cr.custom_iso", &[("name", name.clone())]);
                iso_rows.push(self.iso_row(
                    Some(custom.clone()),
                    &name,
                    &subtitle,
                    profile.icon,
                    cx,
                ));
            }
        }

        let mut arch_chips: Vec<Stateful<Div>> = Vec::new();
        for arch in backend::ARCHES {
            if !backend::arch_available(arch) && self.form.arch != arch {
                continue;
            }
            let id = match arch {
                "i686" => "form_arch_i686",
                "aarch64" => "form_arch_aarch64",
                _ => "form_arch_x86_64",
            };
            let label = match arch {
                "i686" => "i686 (32-bit)",
                "aarch64" => "ARM64 (aarch64)",
                _ => "x86_64",
            };
            let selected = self.form.arch == arch;
            arch_chips.push(
                button(
                    id,
                    label,
                    if selected {
                        BtnKind::Accent
                    } else {
                        BtnKind::Normal
                    },
                )
                .h(px(28.))
                .on_click(
                    cx.listener(move |this, _: &ClickEvent, _, cx| this.form_select_arch(arch, cx)),
                ),
            );
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
        let focused = self.name_focus.is_focused(window);
        let cursor = if self.form.name.is_char_boundary(self.name_cursor) {
            self.name_cursor
        } else {
            self.form.name.len()
        };
        let before = self.form.name[..cursor].to_string();
        let after = self.form.name[cursor..].to_string();
        let empty = self.form.name.is_empty();
        let name_mode = if self.name_manual {
            self.t("cr.name_manual")
        } else {
            self.t("cr.name_auto")
        };
        let disk_mode = if self.form.iso.is_some() {
            self.t("cr.boot_cd")
        } else {
            self.t("cr.empty")
        };
        let disk_path = self.tf(
            "cr.disk_path",
            &[
                ("name", self.form.name.clone()),
                ("mode", disk_mode.to_string()),
            ],
        );

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
                                            .child(self.t("cr.title")),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(col(MUTED))
                                            .child(self.t("cr.subtitle")),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(div().text_size(px(10.)).text_color(col(MUTED)).child(
                                        self.tf(
                                            "cr.host",
                                            &[
                                                ("cpu", total_cpu.to_string()),
                                                ("ram", fmt_mb(total_ram)),
                                            ],
                                        ),
                                    ))
                                    .child(
                                        button(
                                            "iso_pick",
                                            self.t("cr.choose_iso"),
                                            BtnKind::Normal,
                                        )
                                        .on_click(
                                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.form_pick_iso(cx)
                                            }),
                                        ),
                                    )
                                    .child(
                                        button("iso_rescan", self.t("cr.rescan"), BtnKind::Normal)
                                            .on_click(cx.listener(
                                                |this, _: &ClickEvent, _, cx| this.form_rescan(cx),
                                            )),
                                    ),
                            ),
                    )
                    .child(section_label(self.t("cr.media")))
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
                            .child(self.t("cr.iso_hint")),
                    )
                    .child(section_label(self.t("cr.arch")))
                    .child(div().flex().items_center().gap(px(8.)).children(arch_chips))
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(col(MUTED))
                            .child(self.t("cr.arch_hint")),
                    )
                    .child(section_label(self.t("cr.os")))
                    .child(div().flex().flex_col().gap(px(8.)).children(grid_rows))
                    .child(section_label(self.t("cr.name")))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.))
                                            .flex()
                                            .items_center()
                                            .gap(px(10.))
                                            .px(px(12.))
                                            .h(px(40.))
                                            .rounded(px(8.))
                                            .bg(col(CARD))
                                            .border_1()
                                            .border_color(col(if focused {
                                                ACCENT
                                            } else {
                                                BORDER
                                            }))
                                            .id("name_input")
                                            .track_focus(&self.name_focus)
                                            .cursor(CursorStyle::IBeam)
                                            .on_click(cx.listener(
                                                |this, _: &ClickEvent, window, cx| {
                                                    this.focus_name(window, cx)
                                                },
                                            ))
                                            .on_key_down(cx.listener(Self::name_key_down))
                                            .child(
                                                div()
                                                    .text_size(px(10.))
                                                    .text_color(col(MUTED))
                                                    .child(self.t("cr.name_field")),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .min_w(px(0.))
                                                    .text_size(px(13.))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(col(if empty {
                                                        MUTED
                                                    } else {
                                                        TEXT
                                                    }))
                                                    .child(if empty {
                                                        "vm".to_string()
                                                    } else {
                                                        before
                                                    })
                                                    .child(if focused {
                                                        div().w(px(2.)).h(px(16.)).bg(col(ACCENT))
                                                    } else {
                                                        div()
                                                    })
                                                    .child(after),
                                            )
                                            .child(div().flex_1())
                                            .child(
                                                div()
                                                    .text_size(px(9.))
                                                    .text_color(col(if self.name_manual {
                                                        ACCENT
                                                    } else {
                                                        MUTED
                                                    }))
                                                    .child(name_mode),
                                            ),
                                    )
                                    .child(
                                        button(
                                            "name_reset",
                                            self.t("cr.name_reset"),
                                            BtnKind::Normal,
                                        )
                                        .on_click(
                                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.form_reset_name(cx)
                                            }),
                                        ),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(col(MUTED))
                                    .child(self.t("cr.name_hint")),
                            ),
                    )
                    .child(section_label(self.t("cr.hardware")))
                    .child(
                        div()
                            .flex()
                            .gap(px(10.))
                            .child(self.create_stepper(
                                self.t("hw.vcpu"),
                                self.form.vcpu.to_string(),
                                &self.tf("cr.max", &[("max", total_cpu.to_string())]),
                                "form_vcpu_minus",
                                "form_vcpu_plus",
                                HwField::Vcpu,
                                cx,
                            ))
                            .child(self.create_stepper(
                                self.t("hw.memory"),
                                fmt_mb(self.form.ram),
                                &self.tf("cr.max", &[("max", fmt_mb(total_ram))]),
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
                                            .child(self.t("cr.disk")),
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
                                                            .on_click(cx.listener(
                                                                |this, _: &ClickEvent, _, cx| {
                                                                    this.form_disk_step(-1, cx)
                                                                },
                                                            )),
                                                    )
                                                    .child(
                                                        step_button("form_disk_plus", "＋")
                                                            .on_click(cx.listener(
                                                                |this, _: &ClickEvent, _, cx| {
                                                                    this.form_disk_step(1, cx)
                                                                },
                                                            )),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(col(MUTED))
                                            .child(self.t("cr.disk_hint")),
                                    ),
                            ),
                    )
                    .child(
                        toggle_row(
                            self.t("cr.kvm"),
                            if !backend::arch_native(&self.form.arch) {
                                self.t("hw.kvm_foreign")
                            } else if self.kvm_ok {
                                self.t("cr.kvm_hint")
                            } else {
                                self.t("hw.kvm_missing")
                            },
                            self.form.kvm,
                        )
                        .id("form_kvm")
                        .on_click(cx.listener(
                            |this, _: &ClickEvent, _, cx| {
                                this.form.kvm = !this.form.kvm
                                    && this.kvm_ok
                                    && backend::arch_native(&this.form.arch);
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(col(MUTED))
                            .child(disk_path),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap(px(8.))
                            .pt(px(2.))
                            .child(
                                button("create_cancel", self.t("cr.cancel"), BtnKind::Normal)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.nav(Screen::Vms, cx)
                                    })),
                            )
                            .child(
                                button("create_go", self.t("cr.create"), BtnKind::Accent)
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
        path: Option<PathBuf>,
        title: &str,
        subtitle: &str,
        icon: &str,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let selected = self.form.iso.as_ref() == path.as_ref();
        let click_path = path.clone();
        div()
            .id(SharedString::from(format!(
                "iso-{}",
                path.as_ref()
                    .map(|path| path.to_string_lossy().to_string())
                    .unwrap_or_else(|| "none".into())
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
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.form_select_iso(click_path.clone(), cx)
            }))
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
                    .child(div().pt(px(6.)).child(
                        button("placeholder_back", self.t("ph.back"), BtnKind::Accent).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| this.nav(Screen::Vms, cx)),
                        ),
                    )),
            )
    }

    fn confirm_overlay(&self, dialog: &ConfirmDialog, cx: &mut Context<Self>) -> Div {
        div()
            .absolute()
            .top(px(0.))
            .left(px(0.))
            .right(px(0.))
            .bottom(px(0.))
            .bg(alpha(col(0x000000), 0.62))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(470.))
                    .p(px(24.))
                    .rounded(px(12.))
                    .bg(col(PANEL))
                    .border_1()
                    .border_color(col(BORDER))
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(div().text_size(px(26.)).text_color(col(AMBER)).child("⚠"))
                    .child(
                        div()
                            .text_size(px(17.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(dialog.title.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(col(MUTED))
                            .child(dialog.body.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap(px(8.))
                            .pt(px(4.))
                            .child(
                                button("confirm_cancel", self.t("cr.cancel"), BtnKind::Normal)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.confirm = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                button("confirm_yes", &dialog.confirm, BtnKind::DangerSolid)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.resolve_confirm(cx)
                                    })),
                            ),
                    ),
            )
    }

    fn resolve_confirm(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.confirm.take() else {
            return;
        };
        if dialog.opened_at.elapsed() < Duration::from_millis(500) {
            self.confirm = Some(dialog);
            return;
        }
        match dialog.action {
            ConfirmAction::DeleteVm(name) => self.delete_selected_now(&name, cx),
            ConfirmAction::RemoveDisk(target) => self.disk_remove_now(&target, cx),
            ConfirmAction::RemoveNic(mac) => self.nic_remove_now(&mac, cx),
        }
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
            None => (self.t("status.ready").to_string(), col(MUTED)),
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
                    .child("GPL-3.0 · Rust"),
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
        if self.name_focus_pending && self.screen == Screen::Create {
            self.name_focus_pending = false;
            window.focus(&self.name_focus);
            self.name_cursor = self.form.name.len();
            self.name_fresh = !self.name_manual;
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
                    .child(self.body(window, cx))
                    .child(self.status_bar(cx)),
            )
            .children(
                self.confirm
                    .clone()
                    .map(|dialog| self.confirm_overlay(&dialog, cx)),
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
        BtnKind::Danger | BtnKind::Normal => (col(CARD), col(TEXT), col(BORDER)),
        BtnKind::DangerSolid => (col(RED), col(0x2a0606), col(RED)),
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
            BtnKind::DangerSolid => this.bg(col(0xdc2626)).border_color(col(0xdc2626)),
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

impl VmmApp {
    fn state_name(&self, vm: &Vm) -> &'static str {
        self.t(state_key(vm))
    }

    fn state_badge(&self, vm: &Vm) -> Div {
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
                    .child(self.state_name(vm)),
            )
    }
}

fn state_key(vm: &Vm) -> &'static str {
    match vm.state {
        1 => "state.running",
        3 => "state.paused",
        4 => "state.shutdown",
        6 => "state.crashed",
        7 => "state.suspended",
        _ => "state.shutoff",
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

fn prev_boundary(text: &str, index: usize) -> usize {
    if index == 0 {
        return 0;
    }
    let mut position = index - 1;
    while position > 0 && !text.is_char_boundary(position) {
        position -= 1;
    }
    position
}

fn next_boundary(text: &str, index: usize) -> usize {
    if index >= text.len() {
        return text.len();
    }
    let mut position = index + 1;
    while position < text.len() && !text.is_char_boundary(position) {
        position += 1;
    }
    position
}
