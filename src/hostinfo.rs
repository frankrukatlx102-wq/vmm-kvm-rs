use std::fs;

pub fn host_limits() -> (u32, u32) {
    let mut ram_mb = 1024u32;
    if let Ok(text) = fs::read_to_string("/proc/meminfo") {
        for line in text.lines() {
            if line.starts_with("MemTotal:") {
                if let Some(kb) = line.split_whitespace().nth(1) {
                    if let Ok(kb) = kb.parse::<u64>() {
                        ram_mb = (kb / 1024) as u32;
                    }
                }
                break;
            }
        }
    }
    let cpus = std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(1);
    (ram_mb, cpus)
}

pub fn kvm_available() -> bool {
    std::path::Path::new("/dev/kvm").exists()
}
