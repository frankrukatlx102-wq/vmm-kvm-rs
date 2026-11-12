import json
import os
import sys
import time
import xml.etree.ElementTree as ET

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
python_dir = sys.argv[1] if len(sys.argv) > 1 else os.environ.get("VMM_PYTHON_DIR", "../vmm-kvm")
sys.path.insert(0, python_dir)

from vmm_kvm.ui.create_dialog import _auto_name, _detect

FILES = [
    "ubuntu-24.04-desktop-amd64.iso",
    "debian-13.7.0-amd64-netinst.iso",
    "Fedora-Workstation-Live-42.iso",
    "archlinux-2026.11.01-x86_64.iso",
    "openSUSE-Leap-16.0-DVD-x86_64.iso",
    "Windows-11_24H2_x64.iso",
    "windows-10-22H2-amd64.iso",
    "FreeBSD-15.0-RELEASE-amd64-dvd1.iso",
    "linuxmint-23-cinnamon-64bit.iso",
    "rocky-10.0-x86_64-dvd.iso",
]

for _ in range(1000):
    for name in FILES:
        osid, version = _detect(name)
        _auto_name(osid, version, name)

iso_iterations = 20000
iso_start = time.perf_counter_ns()
for _ in range(iso_iterations):
    for name in FILES:
        osid, version = _detect(name)
        _auto_name(osid, version, name)
iso_ops = iso_iterations * len(FILES)
iso_ns = (time.perf_counter_ns() - iso_start) / iso_ops

xml_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "domain.xml")
xml = open(xml_path, encoding="utf-8").read()
for _ in range(100):
    ET.fromstring(xml)

xml_iterations = 20000
xml_start = time.perf_counter_ns()
for _ in range(xml_iterations):
    ET.fromstring(xml)
xml_ns = (time.perf_counter_ns() - xml_start) / xml_iterations

print(json.dumps({
    "iso_ops": iso_ops,
    "iso_ns_per_op": round(iso_ns, 1),
    "xml_ops": xml_iterations,
    "xml_ns_per_op": round(xml_ns, 1),
}))
