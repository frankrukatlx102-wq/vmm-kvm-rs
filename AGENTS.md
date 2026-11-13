# AGENTS.md

## Repository

- Rust 2024, single binary `vmm-kvm-rs`: gpui 0.2.2 UI plus a direct libvirt backend. Linux only.
- Separate git repository from the PySide6 reference app at `../vmm-kvm`; never mix histories or copy changes blindly.
- License is GPL-3.0-only. Keep `LICENSE`, `Cargo.toml` metadata and the README badge consistent.
- Owner convention: no comments, doc comments, TODO/FIXME markers or `#[allow(dead_code)]` in source. Keep the build warning-free.

## Commands

- `cargo build` / `cargo build --release`
- `cargo fmt --check` / `cargo clippy`
- `cargo run -- --selftest` is the real integration test. It creates `vmmself-rs-*` VMs, tests BIOS/UEFI/i686/ARM64, hardware, disks, network, snapshots and architecture conversion, then cleans up.
- `cargo run --release -- --bench` runs the ISO/XML microbenchmarks.
- `python3 benchmarks/python_cpu.py ../vmm-kvm` runs the Python comparison benchmark.
- There is no `cargo test` suite; a bare `cargo test` does not exercise the backend.

## Safety

- Test only VMs named `vmmself-rs-*`. Never stop, undefine or delete another VM unless the user explicitly asks for that specific VM.
- Do not put real VM names, usernames or home paths into screenshots, README images or commits. Screenshots use demo data.
- Do not launch interactive/fullscreen UI tests on the user's active desktop without need; it steals focus. Prefer demo builds and ask before disruptive runs.
- `pkill -f <pattern>` can kill the invoking shell when the pattern appears in its own command line. Use bracketed patterns or exact PIDs.
- Self-test cleanup must cover VMs, disk directories, temporary ISOs and ARM NVRAM. `delete()` already passes NVRAM, snapshot and managed-save undefine flags.

## Structure and flow

- `src/main.rs`: window, assets, `--selftest` and `--bench`.
- `src/ui.rs`: all gpui state, render and handlers.
- `src/backend.rs`: libvirt connection, XML edits, disks, networks, snapshots, architecture/machine conversion.
- `src/i18n.rs`: five-language string table. `src/osdata.rs`: 39 OS profiles and ISO detection. `src/hostinfo.rs`: host limits.
- Backend connects to `qemu:///session` first, then `qemu:///system`. Persistent changes edit inactive XML and `define`; live changes are best-effort.
- `VmRequest` is the VM-create contract; `Hw` is the hardware-edit contract. Add fields to `hw_read`, `hw_apply`, create XML and UI together.
- Architectures are `x86_64`, `i686`, `aarch64`. `machine_for_arch` maps q35 / pc / virt; `arch_native` gates KVM. Foreign architectures run under TCG, and ARM forces UEFI plus virtio video.
- Architecture and machine changes are allowed only while the VM is stopped. Route them through `convert_arch_devices` and `convert_machine_devices`; they remove incompatible controllers, addresses, q35 watchdogs and ARM GIC, and convert IDE ↔ SATA. Do not bypass them.
- `define_domain_with_uefi` handles x86 OVMF and AArch64 firmware fallback. New architectures need firmware discovery, device conversion and selftest coverage.

## UI and localization gotchas

- Every user-facing string belongs in `src/i18n.rs` and must be filled for all five languages: English, Russian, Chinese, Portuguese (BR), Spanish. Unknown keys render as `?`.
- gpui 0.2.2 `Window::prompt` is unimplemented on Linux. Destructive actions use the in-app `ConfirmDialog` overlay.
- VM name editing is custom key handling (`name_key_down`), not a gpui input widget; preserve UTF-8 cursor boundaries.
- Avoid `flex_wrap` for auto-height multi-row grids; this gpui version mis-measures them. Use explicit rows like the OS grid.
- Asset lookup order: next to the executable, then `$XDG_DATA_HOME/vmm-kvm/assets`, then repo `assets/`. New OS profiles need an SVG in `assets/icons/os/`.

## Release and docs

- Keep the version in `Cargo.toml` and `Cargo.lock`, `CHANGELOG.md`, README release notes and the git tag in sync.
- Current release: `0.1.0-alpha`, tag `v0.1-alpha`, GitHub pre-release.
- Do not change benchmark numbers without rerunning `--bench` and `benchmarks/python_cpu.py`; the README documents the exact methodology.
- README screenshots in `docs/screenshots` were captured with demo data; regenerate only with demo VMs.
