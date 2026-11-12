# VMM-KVM (Rust)

A native Linux QEMU/KVM manager built with [gpui](https://github.com/zed-industries/zed/tree/main/crates/gpui) and libvirt. This is the Rust port of [vmm-kvm](https://github.com/frankrukatlx102-wq/vmm-kvm); the PySide6 application remains the feature-complete reference implementation.

Dark teal interface, VM cards with live CPU/RAM sparklines, one-screen VM creation and hardware editing.

## Features

- Connects to `qemu:///session` first, falls back to `qemu:///system`
- VM list with state badges and live CPU/RAM sparklines
- Start, graceful shutdown, force off, reboot, pause/resume
- SPICE/VNC console via `virt-viewer` (or `remote-viewer`)
- One-screen creation: ISO list, local ISO picker, automatic OS detection, 39 OS profiles, host-capped vCPU/RAM, disk size and KVM toggle
- Inline VM name editing with UTF-8 support and automatic unique names
- Hardware tab: vCPU, RAM, KVM, UEFI, TPM 2.0, video model, autostart
- Disks tab: attach/eject ISO, add and remove qcow2 disks
- Network tab: add and remove user-mode or bridged interfaces
- Snapshots tab: create, revert and delete snapshots
- UEFI domain definitions with an OVMF fallback for older libvirt versions
- Safe deletion: modal confirmation, running-VM guard and automatic `domain.xml.bak`
- Five languages with live switching: English, Russian, Chinese, Portuguese (BR), Spanish
- OS icons, a custom app icon and a dark teal theme

Only libvirt session VMs are touched. The optional self-test creates VMs named `vmmself-rs-*` and removes them afterwards.

## Requirements

- Linux with libvirt and QEMU/KVM
- `qemu-img` for disk creation
- `virt-viewer` or `remote-viewer` for the graphical console
- `xdg-desktop-portal` for the local ISO picker (installed by most desktops; `zenity` is used as a fallback)
- a CJK font such as Noto Sans CJK for the Chinese interface (common on most desktop distributions)
- Rust toolchain (edition 2024)
- Development packages for libvirt, Vulkan, Wayland/X11, fontconfig and freetype

Package names vary by distribution. Examples:

```bash
# Debian / Ubuntu
sudo apt install build-essential pkg-config libvirt-dev libssl-dev \
  libxkbcommon-dev libwayland-dev libvulkan-dev libfontconfig1-dev \
  libfreetype-dev qemu-system-x86 qemu-utils virt-viewer

# Fedora
sudo dnf install gcc gcc-c++ make pkgconf-pkg-config libvirt-devel openssl-devel \
  libxkbcommon-devel wayland-devel vulkan-loader-devel fontconfig-devel \
  freetype-devel qemu-kvm qemu-img virt-viewer

# Arch Linux
sudo pacman -S --needed base-devel pkgconf libvirt openssl libxkbcommon \
  wayland vulkan-icd-loader fontconfig freetype2 qemu-full virt-viewer
```

## Build and run

```bash
cargo build --release
./target/release/vmm-kvm-rs
```

During development:

```bash
cargo run
```

The UI loads OS icons from `assets/icons/os`. When running the binary outside the project tree, keep an `assets` directory next to the executable (the app also falls back to the build-time path during development).

## Self-test

```bash
cargo run -- --selftest
```

The test creates a BIOS VM and a UEFI/q35 VM with unique `vmmself-rs-*` names, checks creation, hardware read/write, start, statistics and deletion, then cleans up.

## Status

Implemented: VM list, overview, hardware, disks, networks, snapshots, console, local ISO picker, manual VM names, live statistics and five-language UI.

Roadmap: storage pools, SSH hosts, VM cloning and import, and remote libvirt connections.

## License

GPL-3.0. See [LICENSE](LICENSE).
