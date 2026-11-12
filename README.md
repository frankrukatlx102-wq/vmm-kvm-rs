# VMM-KVM (Rust)

Rust port of VMM-KVM, a QEMU/KVM virtual machine manager for Linux. Built with gpui (Zed UI framework) and libvirt.

Work in progress. Core first: VM list, create, start and stop, hardware settings. Then disks, networks, snapshots, storage pools, SSH hosts and localization.

## Build

```bash
cargo build --release
./target/release/vmm-kvm-rs
```

## Status

- OS profiles and ISO auto-detection: done
- Host limits (RAM, CPU, KVM): done
- libvirt layer, UI and VM creation: in progress

## License

GPL-3.0. See [LICENSE](LICENSE).
