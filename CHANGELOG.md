# Changelog

## v0.1 alpha — 2026-11-13

First public release.

### Added

- x86_64, i686 and ARM64 guest architecture selection
- architecture and machine-type conversion for stopped VMs
- q35 / i440fx chipset selector with IDE/SATA and controller conversion
- ARM64 `virt` machine support with automatic AArch64 UEFI firmware
- VM creation with ISO auto-detection and host-capped resources
- hardware editing, disk management, ISO attach/eject and network interfaces
- snapshots: create, revert and delete
- five-language UI with live switching: English, Russian, Chinese, Portuguese (BR), Spanish
- custom application icon
- safe deletion with a confirmation modal, running-VM guard and XML backup
- reproducible Python vs Rust benchmarks and `--bench` mode

### Fixed

- legacy i440fx VMs no longer break when UEFI forced a q35 switch
- q35-only watchdog and GIC features are removed when returning to x86
- ARM NVRAM is cleaned on undefine

### Known limitations

- ARM64 runs under TCG on x86 hosts
- 32-bit x86 UEFI is not enabled
- changing architecture requires reinstalling the guest OS
- storage pools, SSH hosts and VM import are not implemented yet
