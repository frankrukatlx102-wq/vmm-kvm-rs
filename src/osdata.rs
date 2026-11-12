#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Windows,
    Linux,
    Bsd,
    Other,
}

pub struct OsProfile {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub family: Family,
    pub vcpu: u32,
    pub ram_mb: u32,
    pub disk_gb: u32,
    pub uefi: bool,
    pub tpm: bool,
    pub machine: &'static str,
    pub bus: &'static str,
    pub net: &'static str,
    pub video: &'static str,
}

macro_rules! os {
    ($id:expr, $name:expr, $icon:expr, $fam:ident, $vcpu:expr, $ram:expr, $disk:expr,
     $uefi:expr, $tpm:expr, $machine:expr, $bus:expr, $net:expr, $video:expr) => {
        OsProfile {
            id: $id,
            name: $name,
            icon: $icon,
            family: Family::$fam,
            vcpu: $vcpu,
            ram_mb: $ram,
            disk_gb: $disk,
            uefi: $uefi,
            tpm: $tpm,
            machine: $machine,
            bus: $bus,
            net: $net,
            video: $video,
        }
    };
}

macro_rules! linux {
    ($id:expr, $name:expr, $icon:expr, $vcpu:expr, $ram:expr, $disk:expr, $uefi:expr) => {
        os!($id, $name, $icon, Linux, $vcpu, $ram, $disk, $uefi, false, "q35",
            "virtio", "virtio", "virtio")
    };
}

pub const OS_LIST: &[OsProfile] = &[
    os!("windows-11", "Windows 11", "windows-11", Windows, 4, 4096, 64, true, true,
        "q35", "sata", "e1000e", "qxl"),
    os!("windows-10", "Windows 10", "windows-10", Windows, 4, 4096, 64, false, false,
        "pc", "sata", "e1000e", "qxl"),
    os!("windows-8", "Windows 8", "windows-8", Windows, 2, 2048, 40, false, false,
        "pc", "sata", "e1000e", "qxl"),
    os!("windows-8.1", "Windows 8.1", "windows-8", Windows, 2, 2048, 40, false, false,
        "pc", "sata", "e1000e", "qxl"),
    os!("windows-7", "Windows 7", "windows-7", Windows, 2, 2048, 40, false, false,
        "pc", "ide", "e1000", "vga"),
    os!("windows-vista", "Windows Vista", "windows-vista", Windows, 2, 2048, 40, false,
        false, "pc", "ide", "e1000", "vga"),
    os!("windows-xp", "Windows XP", "windows-xp", Windows, 1, 1024, 20, false, false,
        "pc", "ide", "rtl8139", "vga"),
    os!("windows-2000", "Windows 2000", "windows-flag", Windows, 1, 512, 16, false,
        false, "pc", "ide", "rtl8139", "vga"),
    os!("windows-me", "Windows ME", "windows-flag", Windows, 1, 256, 10, false, false,
        "pc", "ide", "rtl8139", "vga"),
    os!("windows-98", "Windows 98", "windows-98", Windows, 1, 256, 10, false, false,
        "pc", "ide", "rtl8139", "vga"),
    os!("windows-95", "Windows 95", "windows-95", Windows, 1, 128, 8, false, false,
        "pc", "ide", "rtl8139", "vga"),
    os!("windows-nt", "Windows NT", "windows-nt", Windows, 1, 128, 8, false, false,
        "pc", "ide", "rtl8139", "vga"),
    linux!("ubuntu", "Ubuntu", "ubuntu", 2, 2048, 20, true),
    linux!("debian", "Debian", "debian", 2, 2048, 20, true),
    linux!("fedora", "Fedora", "fedora", 2, 4096, 30, true),
    linux!("arch", "Arch Linux", "archlinux", 2, 2048, 20, true),
    linux!("opensuse", "openSUSE", "opensuse", 2, 4096, 30, true),
    linux!("astra", "Astra Linux", "astra-linux", 2, 2048, 20, true),
    linux!("void", "Void Linux", "voidlinux", 2, 2048, 20, false),
    linux!("gentoo", "Gentoo", "gentoo", 2, 2048, 20, false),
    linux!("mint", "Linux Mint", "linuxmint", 2, 4096, 20, true),
    linux!("manjaro", "Manjaro", "manjaro", 2, 4096, 20, true),
    linux!("nixos", "NixOS", "nixos", 2, 4096, 20, true),
    linux!("endeavouros", "EndeavourOS", "endeavouros", 2, 2048, 20, true),
    linux!("almalinux", "AlmaLinux", "almalinux", 2, 2048, 20, false),
    linux!("rocky", "Rocky Linux", "rockylinux", 2, 2048, 20, false),
    linux!("centos", "CentOS", "centos", 2, 2048, 20, false),
    linux!("slackware", "Slackware", "slackware", 2, 2048, 20, false),
    linux!("zorin", "Zorin OS", "zorin", 2, 4096, 20, true),
    linux!("popos", "Pop!_OS", "popos", 2, 4096, 30, true),
    linux!("cachyos", "CachyOS", "cachyos", 2, 4096, 20, true),
    linux!("mxlinux", "MX Linux", "mxlinux", 2, 2048, 20, false),
    linux!("garuda", "Garuda Linux", "garudalinux", 2, 4096, 40, true),
    linux!("linux", "Linux (generic)", "linux", 2, 2048, 20, true),
    os!("freebsd", "FreeBSD", "freebsd", Bsd, 2, 2048, 20, true, false, "q35",
        "virtio", "virtio", "vga"),
    os!("openbsd", "OpenBSD", "openbsd", Bsd, 2, 2048, 20, false, false, "pc",
        "virtio", "virtio", "vga"),
    os!("netbsd", "NetBSD", "netbsd", Bsd, 2, 2048, 20, false, false, "pc",
        "virtio", "virtio", "vga"),
    os!("android", "Android x86", "android", Other, 2, 2048, 8, false, false, "pc",
        "sata", "e1000", "vga"),
    os!("other", "Other", "other", Other, 2, 2048, 20, false, false, "pc",
        "virtio", "virtio", "virtio"),
];

pub fn find(id: &str) -> &'static OsProfile {
    OS_LIST.iter().find(|o| o.id == id).unwrap_or(&OS_LIST[OS_LIST.len() - 1])
}

pub fn guess(name: &str) -> &'static OsProfile {
    let s = name.to_lowercase();
    let rules: &[(&str, &str)] = &[
        ("win 11", "windows-11"), ("windows 11", "windows-11"), ("win11", "windows-11"),
        ("win 10", "windows-10"), ("windows 10", "windows-10"), ("win10", "windows-10"),
        ("win 8.1", "windows-8.1"), ("windows 8.1", "windows-8.1"),
        ("win 8", "windows-8"), ("windows 8", "windows-8"),
        ("win 7", "windows-7"), ("windows 7", "windows-7"), ("win7", "windows-7"),
        ("vista", "windows-vista"),
        ("xp", "windows-xp"),
        ("2000", "windows-2000"),
        ("millennium", "windows-me"), ("windows me", "windows-me"),
        ("windows 98", "windows-98"), ("win98", "windows-98"),
        ("windows 95", "windows-95"), ("win95", "windows-95"),
        ("windows nt", "windows-nt"),
        ("ubuntu", "ubuntu"), ("debian", "debian"), ("fedora", "fedora"),
        ("arch", "arch"), ("opensuse", "opensuse"), ("suse", "opensuse"),
        ("astra", "astra"), ("void", "void"), ("gentoo", "gentoo"),
        ("mint", "mint"), ("manjaro", "manjaro"), ("nixos", "nixos"),
        ("endeavour", "endeavouros"), ("alma", "almalinux"), ("rocky", "rocky"),
        ("centos", "centos"), ("slackware", "slackware"), ("zorin", "zorin"),
        ("pop! os", "popos"), ("pop os", "popos"), ("popos", "popos"),
        ("cachyos", "cachyos"), ("mx linux", "mxlinux"), ("garuda", "garuda"),
        ("freebsd", "freebsd"), ("openbsd", "openbsd"), ("netbsd", "netbsd"),
        ("android", "android"),
    ];
    for (needle, id) in rules {
        if s.contains(needle) {
            return find(id);
        }
    }
    find("other")
}

pub fn detect_from_iso(path: &str) -> (&'static OsProfile, String) {
    let file = path.rsplit('/').next().unwrap_or(path);
    let profile = guess(file);
    let mut clean = file.to_lowercase();
    for noise in ["x86_64", "amd64", "x64", "i386", "i686", "arm64", "aarch64"] {
        clean = clean.replace(noise, " ");
    }
    let version = first_version(&clean).unwrap_or_default();
    (profile, version)
}

fn first_version(s: &str) -> Option<String> {
    let bytes: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i + 2 < bytes.len() {
        if bytes[i].is_ascii_digit() && bytes[i + 1] == '.' && bytes[i + 2].is_ascii_digit() {
            let mut j = i;
            while j < bytes.len() && (bytes[j].is_ascii_digit() || bytes[j] == '.') {
                j += 1;
            }
            let v: String = bytes[i..j].iter().collect();
            return Some(v.trim_end_matches('.').to_string());
        }
        i += 1;
    }
    None
}

pub fn auto_name(profile: &OsProfile, version: &str) -> String {
    if profile.id == "other" {
        String::new()
    } else if version.is_empty() {
        profile.id.to_string()
    } else {
        format!("{}-{}", profile.id, version)
    }
}
