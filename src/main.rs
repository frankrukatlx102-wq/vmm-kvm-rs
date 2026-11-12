#![allow(dead_code)]

mod backend;
mod hostinfo;
mod i18n;
mod osdata;
mod ui;

use std::borrow::Cow;
use std::path::PathBuf;

use anyhow::Result;
use gpui::{
    App, AppContext, Application, AssetSource, Bounds, SharedString, TitlebarOptions, WindowBounds,
    WindowOptions, px, size,
};

struct Assets {
    base: PathBuf,
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        std::fs::read(self.base.join(path))
            .map(|data| Some(Cow::Owned(data)))
            .map_err(Into::into)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(std::fs::read_dir(self.base.join(path))?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .map(SharedString::from)
            .collect())
    }
}

fn assets_base() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let dir = dir.join("assets");
            if dir.is_dir() {
                return dir;
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--selftest") {
        std::process::exit(run_selftest());
    }
    Application::new()
        .with_assets(Assets {
            base: assets_base(),
        })
        .run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("VMM-KVM".into()),
                        ..Default::default()
                    }),
                    window_min_size: Some(size(px(1024.), px(680.))),
                    app_id: Some("vmm-kvm".into()),
                    ..Default::default()
                },
                |_, cx| cx.new(ui::VmmApp::new),
            )
            .unwrap();
            cx.activate(true);
        });
}

fn run_selftest() -> i32 {
    use std::time::Duration;

    let name = format!("vmmself-rs-{}", std::process::id());
    println!("selftest: using session libvirt, VM {name}");
    let b = match backend::Backend::connect("qemu:///session") {
        Ok(b) => b,
        Err(e) => {
            println!("FAIL: connect: {e}");
            return 1;
        }
    };
    cleanup_selftest(&b, &name);
    let profile = osdata::find("other");
    if let Err(e) = b.create_vm(&name, profile, 1, 512, 5, "", true) {
        println!("FAIL: create: {e}");
        cleanup_selftest(&b, &name);
        return 1;
    }
    let ok = (|| -> Result<(), String> {
        let vm = b
            .domains()
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|v| v.name == name)
            .ok_or_else(|| "created VM is missing".to_string())?;
        if vm.vcpus != 1 || vm.maxmem_mb != 512 || vm.disk_gb != 5 {
            return Err(format!(
                "created specs mismatch: {} vCPU, {} MiB, {} GiB",
                vm.vcpus, vm.maxmem_mb, vm.disk_gb
            ));
        }
        println!("ok: created (1 vCPU, 512 MiB, 5 GiB)");

        let mut hw = b.hw_read(&name).map_err(|e| e.to_string())?;
        if hw.vcpu != 1 || hw.ram != 512 {
            return Err("hw_read after create is wrong".to_string());
        }
        hw.vcpu = 2;
        hw.ram = 1024;
        hw.video = "virtio".into();
        b.hw_apply(&name, &hw).map_err(|e| e.to_string())?;
        let hw2 = b.hw_read(&name).map_err(|e| e.to_string())?;
        if hw2.vcpu != 2 || hw2.ram != 1024 {
            return Err(format!(
                "hw_apply did not persist: {} vCPU, {} MiB",
                hw2.vcpu, hw2.ram
            ));
        }
        println!("ok: hardware read/write (2 vCPU, 1024 MiB)");

        let target = b.add_disk(&name, 5, "virtio").map_err(|e| e.to_string())?;
        let disks = b.disks(&name).map_err(|e| e.to_string())?;
        if !disks
            .iter()
            .any(|disk| disk.target == target && disk.size_gb == 5)
        {
            return Err("added disk is missing or has the wrong size".to_string());
        }
        let iso = std::env::temp_dir().join("vmmself-rs-selftest.iso");
        std::fs::write(&iso, b"").map_err(|e| e.to_string())?;
        b.attach_iso(&name, Some(&iso.to_string_lossy()))
            .map_err(|e| e.to_string())?;
        let disks = b.disks(&name).map_err(|e| e.to_string())?;
        if !disks
            .iter()
            .any(|disk| disk.device == "cdrom" && disk.file == iso.to_string_lossy())
        {
            return Err("ISO was not attached".to_string());
        }
        b.attach_iso(&name, None).map_err(|e| e.to_string())?;
        b.remove_disk(&name, &target).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&iso);
        let disks = b.disks(&name).map_err(|e| e.to_string())?;
        if disks.iter().any(|disk| disk.target == target) {
            return Err("disk was not removed".to_string());
        }
        println!("ok: disk add/remove and ISO attach/eject");

        let mac = b.add_nic(&name, "virtio").map_err(|e| e.to_string())?;
        let nics = b.nics(&name).map_err(|e| e.to_string())?;
        if !nics.iter().any(|nic| nic.mac == mac) {
            return Err("added interface is missing".to_string());
        }
        b.remove_nic(&name, &mac).map_err(|e| e.to_string())?;
        let nics = b.nics(&name).map_err(|e| e.to_string())?;
        if nics.iter().any(|nic| nic.mac == mac) {
            return Err("interface was not removed".to_string());
        }
        println!("ok: network interface add/remove");

        b.snap_create(&name, "snap-selftest", "selftest")
            .map_err(|e| e.to_string())?;
        let snaps = b.snapshots(&name).map_err(|e| e.to_string())?;
        if !snaps.iter().any(|s| s.name == "snap-selftest") {
            return Err("snapshot was not created".to_string());
        }
        b.snap_delete(&name, "snap-selftest")
            .map_err(|e| e.to_string())?;
        let snaps = b.snapshots(&name).map_err(|e| e.to_string())?;
        if snaps.iter().any(|s| s.name == "snap-selftest") {
            return Err("snapshot was not deleted".to_string());
        }
        println!("ok: snapshot create/list/delete");

        b.start(&name).map_err(|e| e.to_string())?;
        std::thread::sleep(Duration::from_secs(2));
        let state = b
            .domains()
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|v| v.name == name)
            .ok_or_else(|| "VM disappeared after start".to_string())?;
        if !state.running() {
            return Err("VM did not reach running state".to_string());
        }
        println!("ok: start and running state");

        let mut b2 = backend::Backend::connect("qemu:///session").map_err(|e| e.to_string())?;
        let (cpu, mem, max) = b2.stats(&name).map_err(|e| e.to_string())?;
        if max == 0 || mem > max {
            return Err(format!("stats look wrong: cpu {cpu}, mem {mem}, max {max}"));
        }
        println!("ok: stats (cpu {cpu:.1}%, mem {mem}/{max} MiB)");

        b.force_off(&name).map_err(|e| e.to_string())?;
        std::thread::sleep(Duration::from_millis(500));
        println!("ok: force off");
        Ok(())
    })();

    cleanup_selftest(&b, &name);

    let uefi_name = format!("vmmself-rs-uefi-{}", std::process::id());
    cleanup_selftest(&b, &uefi_name);
    let uefi_ok = (|| -> Result<(), String> {
        let profile = osdata::find("ubuntu");
        b.create_vm(&uefi_name, profile, 2, 2048, 10, "", true)
            .map_err(|e| e.to_string())?;
        let hw = b.hw_read(&uefi_name).map_err(|e| e.to_string())?;
        if !hw.uefi {
            return Err("UEFI flag was not persisted".to_string());
        }
        if !hw.machine.contains("q35") {
            return Err(format!("unexpected machine for UEFI: {}", hw.machine));
        }
        println!("ok: UEFI/q35 domain definition");
        Ok(())
    })();
    cleanup_selftest(&b, &uefi_name);

    match (ok, uefi_ok) {
        (Ok(()), Ok(())) => {
            println!("SELFTEST PASS");
            0
        }
        (Err(e), _) => {
            println!("FAIL: {e}");
            println!("SELFTEST FAIL");
            1
        }
        (_, Err(e)) => {
            println!("FAIL: {e}");
            println!("SELFTEST FAIL");
            1
        }
    }
}

fn cleanup_selftest(b: &backend::Backend, name: &str) {
    if let Ok(vms) = b.domains() {
        if let Some(vm) = vms.iter().find(|v| v.name == name) {
            if vm.running() || vm.paused() {
                let _ = b.force_off(name);
            }
            let _ = b.delete(name);
        }
    }
    let dir = backend::home_dir()
        .join("VirtualMachines")
        .join("vms")
        .join(name);
    let _ = std::fs::remove_dir_all(dir);
}
