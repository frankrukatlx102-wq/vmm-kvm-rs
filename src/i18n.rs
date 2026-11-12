use std::fs;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Ru,
    Zh,
    Pt,
    Es,
}

impl Lang {
    pub const ALL: [Lang; 5] = [Lang::En, Lang::Ru, Lang::Zh, Lang::Pt, Lang::Es];

    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ru => "ru",
            Lang::Zh => "zh",
            Lang::Pt => "pt-BR",
            Lang::Es => "es",
        }
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        match code.trim().to_lowercase().as_str() {
            "en" => Some(Lang::En),
            "ru" => Some(Lang::Ru),
            "zh" | "zh-cn" | "zh_cn" => Some(Lang::Zh),
            "pt" | "pt-br" | "pt_br" => Some(Lang::Pt),
            "es" => Some(Lang::Es),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Ru => "Русский",
            Lang::Zh => "中文",
            Lang::Pt => "Português (BR)",
            Lang::Es => "Español",
        }
    }

    fn short(self) -> &'static str {
        match self {
            Lang::En => "EN",
            Lang::Ru => "RU",
            Lang::Zh => "中文",
            Lang::Pt => "PT",
            Lang::Es => "ES",
        }
    }

    fn next(self) -> Lang {
        match self {
            Lang::En => Lang::Ru,
            Lang::Ru => Lang::Zh,
            Lang::Zh => Lang::Pt,
            Lang::Pt => Lang::Es,
            Lang::Es => Lang::En,
        }
    }
}

struct Entry {
    key: &'static str,
    en: &'static str,
    ru: &'static str,
    zh: &'static str,
    pt: &'static str,
    es: &'static str,
}

impl Entry {
    fn get(&self, lang: Lang) -> &'static str {
        match lang {
            Lang::En => self.en,
            Lang::Ru => self.ru,
            Lang::Zh => self.zh,
            Lang::Pt => self.pt,
            Lang::Es => self.es,
        }
    }
}

macro_rules! s {
    ($key:expr, $en:expr, $ru:expr, $zh:expr, $pt:expr, $es:expr) => {
        Entry {
            key: $key,
            en: $en,
            ru: $ru,
            zh: $zh,
            pt: $pt,
            es: $es,
        }
    };
}

const STRINGS: &[Entry] = &[
    s!(
        "app.tagline",
        "QEMU / KVM manager",
        "Менеджер QEMU / KVM",
        "QEMU / KVM 管理器",
        "Gerenciador QEMU / KVM",
        "Gestor QEMU / KVM"
    ),
    s!(
        "nav.vms",
        "Virtual Machines",
        "Виртуальные машины",
        "虚拟机",
        "Máquinas virtuais",
        "Máquinas virtuales"
    ),
    s!(
        "nav.new",
        "New Virtual Machine",
        "Новая ВМ",
        "新建虚拟机",
        "Nova máquina virtual",
        "Nueva máquina virtual"
    ),
    s!(
        "nav.storage",
        "Storage Pools",
        "Хранилища",
        "存储池",
        "Armazenamento",
        "Almacenamiento"
    ),
    s!("nav.networks", "Networks", "Сети", "网络", "Redes", "Redes"),
    s!(
        "nav.ssh",
        "SSH Hosts",
        "SSH-хосты",
        "SSH 主机",
        "Hosts SSH",
        "Hosts SSH"
    ),
    s!(
        "nav.settings",
        "Settings",
        "Настройки",
        "设置",
        "Configurações",
        "Ajustes"
    ),
    s!(
        "hypervisor",
        "HYPERVISOR",
        "ГИПЕРВИЗОР",
        "虚拟机监控器",
        "HIPERVISOR",
        "HIPERVISOR"
    ),
    s!(
        "conn.offline",
        "offline",
        "нет связи",
        "离线",
        "offline",
        "sin conexión"
    ),
    s!(
        "kvm.available",
        "KVM available",
        "KVM доступен",
        "KVM 可用",
        "KVM disponível",
        "KVM disponible"
    ),
    s!(
        "kvm.unavailable",
        "KVM unavailable",
        "KVM недоступен",
        "KVM 不可用",
        "KVM indisponível",
        "KVM no disponible"
    ),
    s!(
        "title.vms",
        "Virtual Machines",
        "Виртуальные машины",
        "虚拟机",
        "Máquinas virtuais",
        "Máquinas virtuales"
    ),
    s!(
        "title.create",
        "Create Virtual Machine",
        "Создание виртуальной машины",
        "创建虚拟机",
        "Criar máquina virtual",
        "Crear máquina virtual"
    ),
    s!(
        "title.storage",
        "Storage Pools",
        "Хранилища",
        "存储池",
        "Pools de armazenamento",
        "Almacenamiento"
    ),
    s!(
        "title.networks",
        "Networks",
        "Сети",
        "网络",
        "Redes",
        "Redes"
    ),
    s!(
        "title.ssh",
        "SSH Hosts",
        "SSH-хосты",
        "SSH 主机",
        "Hosts SSH",
        "Hosts SSH"
    ),
    s!(
        "title.settings",
        "Settings",
        "Настройки",
        "设置",
        "Configurações",
        "Ajustes"
    ),
    s!(
        "subtitle.vms",
        "{n} total · {running} running",
        "всего {n} · запущено {running}",
        "共 {n} 台 · {running} 台运行中",
        "{n} no total · {running} em execução",
        "{n} en total · {running} en ejecución"
    ),
    s!(
        "subtitle.create",
        "One screen, sensible defaults",
        "Один экран, разумные настройки",
        "一个界面，合理默认值",
        "Uma tela, padrões sensatos",
        "Una pantalla, valores razonables"
    ),
    s!(
        "subtitle.storage",
        "libvirt storage volumes",
        "Хранилища libvirt",
        "libvirt 存储卷",
        "Volumes de armazenamento libvirt",
        "Volúmenes de almacenamiento libvirt"
    ),
    s!(
        "subtitle.networks",
        "Virtual networks and bridges",
        "Виртуальные сети и мосты",
        "虚拟网络与网桥",
        "Redes virtuais e pontes",
        "Redes virtuales y puentes"
    ),
    s!(
        "subtitle.ssh",
        "Remote hypervisors and consoles",
        "Удалённые гипервизоры и консоли",
        "远程虚拟化主机与控制台",
        "Hipervisores e consoles remotas",
        "Hipervisores y consolas remotas"
    ),
    s!(
        "subtitle.settings",
        "Application preferences",
        "Параметры приложения",
        "应用偏好设置",
        "Preferências do aplicativo",
        "Preferencias de la aplicación"
    ),
    s!(
        "tb.connected",
        "connected",
        "подключено",
        "已连接",
        "conectado",
        "conectado"
    ),
    s!(
        "tb.offline",
        "offline",
        "нет связи",
        "离线",
        "offline",
        "sin conexión"
    ),
    s!(
        "tb.refresh",
        "Refresh",
        "Обновить",
        "刷新",
        "Atualizar",
        "Actualizar"
    ),
    s!(
        "tb.new",
        "New VM",
        "Новая ВМ",
        "新建虚拟机",
        "Nova VM",
        "Nueva VM"
    ),
    s!(
        "tb.language",
        "Language",
        "Язык",
        "语言",
        "Idioma",
        "Idioma"
    ),
    s!(
        "list.summary",
        "{n} machine(s), {running} running",
        "{n} машин(ы), запущено {running}",
        "{n} 台虚拟机，{running} 台运行中",
        "{n} máquina(s), {running} em execução",
        "{n} máquina(s), {running} en ejecución"
    ),
    s!(
        "list.hint",
        "click a card to inspect",
        "нажмите на карточку",
        "点击卡片查看详情",
        "clique em um cartão para inspecionar",
        "haz clic en una tarjeta para inspeccionar"
    ),
    s!(
        "state.running",
        "Running",
        "Работает",
        "运行中",
        "Em execução",
        "En ejecución"
    ),
    s!(
        "state.paused",
        "Paused",
        "Пауза",
        "已暂停",
        "Pausada",
        "Pausada"
    ),
    s!(
        "state.shutdown",
        "Shutting down",
        "Завершение работы",
        "正在关机",
        "Desligando",
        "Apagando"
    ),
    s!(
        "state.crashed",
        "Crashed",
        "Сбой",
        "已崩溃",
        "Falhou",
        "Fallida"
    ),
    s!(
        "state.suspended",
        "Suspended",
        "Приостановлена",
        "已挂起",
        "Suspensa",
        "Suspendida"
    ),
    s!(
        "state.shutoff",
        "Shut off",
        "Выключена",
        "已关机",
        "Desligada",
        "Apagada"
    ),
    s!(
        "vm.spec",
        "{vcpu} vCPU · {ram} RAM · {disk} disk",
        "{vcpu} vCPU · {ram} RAM · {disk} диск",
        "{vcpu} vCPU · {ram} 内存 · {disk} 磁盘",
        "{vcpu} vCPU · {ram} RAM · {disk} disco",
        "{vcpu} vCPU · {ram} RAM · {disk} disco"
    ),
    s!(
        "tab.overview",
        "Overview",
        "Обзор",
        "概览",
        "Visão geral",
        "Resumen"
    ),
    s!(
        "tab.hardware",
        "Hardware",
        "Железо",
        "硬件",
        "Hardware",
        "Hardware"
    ),
    s!("tab.disks", "Disks", "Диски", "磁盘", "Discos", "Discos"),
    s!("tab.network", "Network", "Сеть", "网络", "Rede", "Red"),
    s!(
        "tab.snapshots",
        "Snapshots",
        "Снапшоты",
        "快照",
        "Snapshots",
        "Instantáneas"
    ),
    s!(
        "ov.os",
        "Operating system",
        "Операционная система",
        "操作系统",
        "Sistema operacional",
        "Sistema operativo"
    ),
    s!("ov.state", "State", "Состояние", "状态", "Estado", "Estado"),
    s!("ov.vcpu", "vCPU", "vCPU", "vCPU", "vCPU", "vCPU"),
    s!(
        "ov.memory",
        "Memory",
        "Память",
        "内存",
        "Memória",
        "Memoria"
    ),
    s!("ov.disk", "Disk", "Диск", "磁盘", "Disco", "Disco"),
    s!(
        "ov.machine",
        "Machine",
        "Машина",
        "机型",
        "Máquina",
        "Máquina"
    ),
    s!(
        "ov.firmware",
        "Firmware",
        "Прошивка",
        "固件",
        "Firmware",
        "Firmware"
    ),
    s!("ov.tpm", "TPM", "TPM", "TPM", "TPM", "TPM"),
    s!("ov.video", "Video", "Видео", "显卡", "Vídeo", "Vídeo"),
    s!(
        "ov.autostart",
        "Autostart",
        "Автозапуск",
        "自动启动",
        "Início automático",
        "Inicio automático"
    ),
    s!(
        "ov.console",
        "Console",
        "Консоль",
        "控制台",
        "Console",
        "Consola"
    ),
    s!("ov.on", "on", "вкл", "开", "ligado", "activado"),
    s!("ov.off", "off", "выкл", "关", "desligado", "desactivado"),
    s!("ov.yes", "yes", "да", "是", "sim", "sí"),
    s!("ov.no", "no", "нет", "否", "não", "no"),
    s!(
        "ov.now",
        "now {value}",
        "сейчас {value}",
        "当前 {value}",
        "agora {value}",
        "ahora {value}"
    ),
    s!("ov.power", "POWER", "ПИТАНИЕ", "电源", "ENERGIA", "ENERGÍA"),
    s!(
        "ov.danger",
        "DANGER ZONE",
        "ОПАСНАЯ ЗОНА",
        "危险操作",
        "ZONA DE PERIGO",
        "ZONA DE PELIGRO"
    ),
    s!(
        "act.start",
        "Start",
        "Запустить",
        "启动",
        "Iniciar",
        "Iniciar"
    ),
    s!(
        "act.shutdown",
        "Shut down",
        "Завершить",
        "关机",
        "Desligar",
        "Apagar"
    ),
    s!(
        "act.forceoff",
        "Force off",
        "Выключить",
        "强制关机",
        "Forçar desligamento",
        "Forzar apagado"
    ),
    s!(
        "act.reboot",
        "Reboot",
        "Перезагрузить",
        "重启",
        "Reiniciar",
        "Reiniciar"
    ),
    s!("act.pause", "Pause", "Пауза", "暂停", "Pausar", "Pausar"),
    s!(
        "act.resume",
        "Resume",
        "Продолжить",
        "继续",
        "Retomar",
        "Reanudar"
    ),
    s!(
        "act.console",
        "Console",
        "Консоль",
        "控制台",
        "Console",
        "Consola"
    ),
    s!(
        "act.delete_vm",
        "Delete virtual machine",
        "Удалить виртуальную машину",
        "删除虚拟机",
        "Excluir máquina virtual",
        "Eliminar máquina virtual"
    ),
    s!(
        "hw.subtitle",
        "Saved to the VM configuration. Running machines keep their current resources until the next start ({vm}).",
        "Сохраняется в конфигурацию ВМ. Работающая машина сохранит текущие ресурсы до следующего запуска ({vm}).",
        "保存到虚拟机配置。运行中的虚拟机在下次启动前继续使用当前资源（{vm}）。",
        "Salvo na configuração da VM. Máquinas em execução mantêm os recursos atuais até a próxima inicialização ({vm}).",
        "Se guarda en la configuración de la VM. Las máquinas en ejecución mantienen los recursos actuales hasta el próximo inicio ({vm})."
    ),
    s!("hw.vcpu", "vCPU", "vCPU", "vCPU", "vCPU", "vCPU"),
    s!(
        "hw.memory",
        "Memory",
        "Память",
        "内存",
        "Memória",
        "Memoria"
    ),
    s!(
        "hw.range_vcpu",
        "1 – {max} (host)",
        "1 – {max} (хост)",
        "1 – {max}（主机）",
        "1 – {max} (host)",
        "1 – {max} (host)"
    ),
    s!(
        "hw.range_ram",
        "256 MiB – {max} (host)",
        "256 МиБ – {max} (хост)",
        "256 MiB – {max}（主机）",
        "256 MiB – {max} (host)",
        "256 MiB – {max} (host)"
    ),
    s!(
        "hw.kvm",
        "KVM acceleration",
        "KVM-ускорение",
        "KVM 加速",
        "Aceleração KVM",
        "Aceleración KVM"
    ),
    s!(
        "hw.kvm_hint",
        "Hardware virtualization",
        "Аппаратная виртуализация",
        "硬件虚拟化",
        "Virtualização por hardware",
        "Virtualización por hardware"
    ),
    s!(
        "hw.kvm_missing",
        "KVM is not available on this host",
        "KVM недоступен на этом хосте",
        "此主机不支持 KVM",
        "KVM não está disponível neste host",
        "KVM no está disponible en este host"
    ),
    s!(
        "hw.uefi",
        "UEFI firmware",
        "Прошивка UEFI",
        "UEFI 固件",
        "Firmware UEFI",
        "Firmware UEFI"
    ),
    s!(
        "hw.uefi_hint",
        "q35 machine is selected automatically",
        "Машина q35 выбирается автоматически",
        "自动选择 q35 机型",
        "A máquina q35 é selecionada automaticamente",
        "La máquina q35 se selecciona automáticamente"
    ),
    s!(
        "hw.tpm", "TPM 2.0", "TPM 2.0", "TPM 2.0", "TPM 2.0", "TPM 2.0"
    ),
    s!(
        "hw.tpm_hint",
        "Required by Windows 11",
        "Требуется для Windows 11",
        "Windows 11 需要",
        "Necessário para Windows 11",
        "Necesario para Windows 11"
    ),
    s!(
        "hw.autostart",
        "Autostart",
        "Автозапуск",
        "自动启动",
        "Início automático",
        "Inicio automático"
    ),
    s!(
        "hw.autostart_hint",
        "Start with the libvirt daemon",
        "Запускать вместе с демоном libvirt",
        "随 libvirt 守护进程启动",
        "Iniciar com o daemon libvirt",
        "Iniciar con el demonio libvirt"
    ),
    s!(
        "hw.video",
        "Video model",
        "Видеомодель",
        "显卡型号",
        "Modelo de vídeo",
        "Modelo de vídeo"
    ),
    s!(
        "hw.video_hint",
        "qxl → virtio → vga",
        "qxl → virtio → vga",
        "qxl → virtio → vga",
        "qxl → virtio → vga",
        "qxl → virtio → vga"
    ),
    s!(
        "hw.change",
        "Change",
        "Изменить",
        "更改",
        "Alterar",
        "Cambiar"
    ),
    s!(
        "hw.apply",
        "Apply hardware changes",
        "Применить изменения железа",
        "应用硬件更改",
        "Aplicar alterações de hardware",
        "Aplicar cambios de hardware"
    ),
    s!(
        "hw.note",
        "Machine, chipset and disk devices are preserved. RAM/vCPU hot-plug depends on the guest.",
        "Машина, чипсет и дисковые устройства сохраняются. Горячее подключение RAM/vCPU зависит от гостя.",
        "机型、芯片组和磁盘设备会保留。RAM/vCPU 热插拔取决于客户机。",
        "Máquina, chipset e dispositivos de disco são preservados. Hot-plug de RAM/vCPU depende do convidado.",
        "Se conservan la máquina, el chipset y los discos. El hot-plug de RAM/vCPU depende del invitado."
    ),
    s!(
        "hw.unavailable",
        "Hardware information is unavailable for this machine.",
        "Информация о железе недоступна для этой машины.",
        "此虚拟机没有可用的硬件信息。",
        "As informações de hardware não estão disponíveis para esta máquina.",
        "La información de hardware no está disponible para esta máquina."
    ),
    s!(
        "snap.title",
        "Snapshots",
        "Снапшоты",
        "快照",
        "Snapshots",
        "Instantáneas"
    ),
    s!(
        "det.no_vm",
        "No virtual machine selected",
        "Виртуальная машина не выбрана",
        "未选择虚拟机",
        "Nenhuma máquina virtual selecionada",
        "Ninguna máquina virtual seleccionada"
    ),
    s!(
        "det.select_vm",
        "Select a machine on the left or create a new one.",
        "Выберите машину слева или создайте новую.",
        "在左侧选择虚拟机或新建一台。",
        "Selecione uma máquina à esquerda ou crie uma nova.",
        "Selecciona una máquina a la izquierda o crea una nueva."
    ),
    s!(
        "snap.count",
        "{n} snapshot(s) · {vm}",
        "{n} снапшот(ов) · {vm}",
        "{n} 个快照 · {vm}",
        "{n} snapshot(s) · {vm}",
        "{n} instantánea(s) · {vm}"
    ),
    s!(
        "snap.create",
        "Create snapshot",
        "Создать снапшот",
        "创建快照",
        "Criar snapshot",
        "Crear instantánea"
    ),
    s!(
        "snap.warn_running",
        "Stop the VM before reverting. Creating a snapshot of a running VM requires qcow2 disks.",
        "Остановите ВМ перед откатом. Для снапшота работающей ВМ нужны диски qcow2.",
        "回滚前请停止虚拟机。为运行中的虚拟机创建快照需要 qcow2 磁盘。",
        "Pare a VM antes de reverter. Criar um snapshot de uma VM em execução exige discos qcow2.",
        "Detén la VM antes de revertir. Crear una instantánea de una VM en ejecución requiere discos qcow2."
    ),
    s!(
        "snap.note",
        "Snapshots capture the disk state. Revert restores it; delete removes only the snapshot.",
        "Снапшоты фиксируют состояние диска. Откат восстанавливает его, удаление убирает только снапшот.",
        "快照记录磁盘状态。回滚会恢复该状态，删除只移除快照。",
        "Snapshots capturam o estado do disco. Reverter restaura; excluir remove apenas o snapshot.",
        "Las instantáneas capturan el estado del disco. Revertir lo restaura; eliminar solo borra la instantánea."
    ),
    s!(
        "snap.empty",
        "No snapshots yet",
        "Снапшотов пока нет",
        "还没有快照",
        "Nenhum snapshot ainda",
        "Aún no hay instantáneas"
    ),
    s!(
        "snap.empty_hint",
        "Create one to freeze the current disk state.",
        "Создайте снапшот, чтобы зафиксировать текущее состояние диска.",
        "创建一个快照来冻结当前磁盘状态。",
        "Crie um para congelar o estado atual do disco.",
        "Crea una para congelar el estado actual del disco."
    ),
    s!(
        "snap.revert",
        "Revert",
        "Откатить",
        "回滚",
        "Reverter",
        "Revertir"
    ),
    s!(
        "snap.delete",
        "Delete",
        "Удалить",
        "删除",
        "Excluir",
        "Eliminar"
    ),
    s!(
        "snap.confirm",
        "Confirm",
        "Подтвердить",
        "确认",
        "Confirmar",
        "Confirmar"
    ),
    s!(
        "snap.created",
        "Snapshot {name} created",
        "Снапшот {name} создан",
        "快照 {name} 已创建",
        "Snapshot {name} criado",
        "Instantánea {name} creada"
    ),
    s!(
        "snap.failed",
        "Snapshot failed: {error}",
        "Ошибка снапшота: {error}",
        "快照失败：{error}",
        "Falha no snapshot: {error}",
        "Error de instantánea: {error}"
    ),
    s!(
        "snap.reverted",
        "Reverted to {name}",
        "Выполнен откат к {name}",
        "已回滚到 {name}",
        "Revertido para {name}",
        "Revertido a {name}"
    ),
    s!(
        "snap.deleted",
        "Deleted snapshot {name}",
        "Снапшот {name} удалён",
        "快照 {name} 已删除",
        "Snapshot {name} excluído",
        "Instantánea {name} eliminada"
    ),
    s!(
        "snap.op_failed",
        "Snapshot operation failed: {error}",
        "Ошибка операции со снапшотом: {error}",
        "快照操作失败：{error}",
        "Falha na operação de snapshot: {error}",
        "Error en la operación de instantánea: {error}"
    ),
    s!("disk.title", "Disks", "Диски", "磁盘", "Discos", "Discos"),
    s!(
        "disk.device",
        "Device",
        "Устройство",
        "设备",
        "Dispositivo",
        "Dispositivo"
    ),
    s!("disk.bus", "Bus", "Шина", "总线", "Barramento", "Bus"),
    s!("disk.size", "Size", "Размер", "大小", "Tamanho", "Tamaño"),
    s!("disk.file", "File", "Файл", "文件", "Arquivo", "Archivo"),
    s!(
        "disk.add",
        "Add disk",
        "Добавить диск",
        "添加磁盘",
        "Adicionar disco",
        "Añadir disco"
    ),
    s!(
        "disk.attach_iso",
        "Attach ISO…",
        "Подключить ISO…",
        "挂载 ISO…",
        "Anexar ISO…",
        "Conectar ISO…"
    ),
    s!(
        "disk.eject_iso",
        "Eject ISO",
        "Извлечь ISO",
        "弹出 ISO",
        "Ejetar ISO",
        "Expulsar ISO"
    ),
    s!(
        "disk.remove",
        "Remove disk",
        "Удалить диск",
        "移除磁盘",
        "Remover disco",
        "Quitar disco"
    ),
    s!(
        "disk.new",
        "New disk",
        "Новый диск",
        "新建磁盘",
        "Novo disco",
        "Nuevo disco"
    ),
    s!(
        "disk.size_label",
        "Size",
        "Размер",
        "大小",
        "Tamanho",
        "Tamaño"
    ),
    s!("disk.bus_label", "Bus", "Шина", "总线", "Barramento", "Bus"),
    s!(
        "disk.create",
        "Create disk",
        "Создать диск",
        "创建磁盘",
        "Criar disco",
        "Crear disco"
    ),
    s!(
        "disk.cancel",
        "Cancel",
        "Отмена",
        "取消",
        "Cancelar",
        "Cancelar"
    ),
    s!(
        "disk.selected",
        "Selected: {target}",
        "Выбран: {target}",
        "已选择：{target}",
        "Selecionado: {target}",
        "Seleccionado: {target}"
    ),
    s!(
        "disk.select_hint",
        "Select a disk to remove it.",
        "Выберите диск, чтобы удалить его.",
        "选择磁盘后可移除。",
        "Selecione um disco para removê-lo.",
        "Selecciona un disco para quitarlo."
    ),
    s!(
        "disk.added",
        "Added disk {target} ({size} GiB)",
        "Добавлен диск {target} ({size} GiB)",
        "已添加磁盘 {target}（{size} GiB）",
        "Disco {target} adicionado ({size} GiB)",
        "Disco {target} añadido ({size} GiB)"
    ),
    s!(
        "disk.add_failed",
        "Add disk failed: {error}",
        "Ошибка добавления диска: {error}",
        "添加磁盘失败：{error}",
        "Falha ao adicionar disco: {error}",
        "Error al añadir disco: {error}"
    ),
    s!(
        "disk.removed",
        "Removed disk {target}",
        "Диск {target} удалён",
        "已移除磁盘 {target}",
        "Disco {target} removido",
        "Disco {target} quitado"
    ),
    s!(
        "disk.remove_failed",
        "Remove disk failed: {error}",
        "Ошибка удаления диска: {error}",
        "移除磁盘失败：{error}",
        "Falha ao remover disco: {error}",
        "Error al quitar disco: {error}"
    ),
    s!(
        "disk.iso_attached",
        "ISO attached",
        "ISO подключён",
        "ISO 已挂载",
        "ISO anexado",
        "ISO conectado"
    ),
    s!(
        "disk.iso_ejected",
        "ISO ejected",
        "ISO извлечён",
        "ISO 已弹出",
        "ISO ejetado",
        "ISO expulsado"
    ),
    s!(
        "disk.iso_failed",
        "ISO operation failed: {error}",
        "Ошибка операции с ISO: {error}",
        "ISO 操作失败：{error}",
        "Falha na operação de ISO: {error}",
        "Error en la operación de ISO: {error}"
    ),
    s!("disk.media", "Media", "Носитель", "介质", "Mídia", "Medio"),
    s!(
        "disk.none",
        "No disks",
        "Нет дисков",
        "没有磁盘",
        "Nenhum disco",
        "No hay discos"
    ),
    s!(
        "disk.running_note",
        "Running VMs apply disk changes after a restart.",
        "Работающая ВМ применит изменения дисков после перезапуска.",
        "运行中的虚拟机将在重启后应用磁盘更改。",
        "VMs em execução aplicam alterações de disco após reiniciar.",
        "Las VM en ejecución aplican los cambios de disco al reiniciar."
    ),
    s!(
        "disk.remove_title",
        "Remove disk {target}?",
        "Удалить диск {target}?",
        "移除磁盘 {target}？",
        "Remover disco {target}?",
        "¿Quitar disco {target}?"
    ),
    s!(
        "disk.remove_body",
        "The disk device will be removed from the VM. The image file stays on disk.",
        "Устройство будет удалено из ВМ. Файл образа останется на диске.",
        "该磁盘设备将从虚拟机移除，镜像文件仍保留在磁盘上。",
        "O dispositivo será removido da VM. O arquivo de imagem permanece no disco.",
        "El dispositivo se quitará de la VM. El archivo de imagen permanece en el disco."
    ),
    s!("net.title", "Network", "Сеть", "网络", "Rede", "Red"),
    s!("net.mac", "MAC", "MAC", "MAC", "MAC", "MAC"),
    s!("net.model", "Model", "Модель", "型号", "Modelo", "Modelo"),
    s!(
        "net.source",
        "Source",
        "Источник",
        "来源",
        "Origem",
        "Origen"
    ),
    s!("net.type", "Type", "Тип", "类型", "Tipo", "Tipo"),
    s!(
        "net.add",
        "Add interface",
        "Добавить интерфейс",
        "添加网卡",
        "Adicionar interface",
        "Añadir interfaz"
    ),
    s!(
        "net.remove",
        "Remove interface",
        "Удалить интерфейс",
        "移除网卡",
        "Remover interface",
        "Quitar interfaz"
    ),
    s!(
        "net.model_label",
        "Model",
        "Модель",
        "型号",
        "Modelo",
        "Modelo"
    ),
    s!(
        "net.add_hint",
        "Session VMs use user-mode networking.",
        "Сессионные ВМ используют user-mode сеть.",
        "会话虚拟机使用用户模式网络。",
        "VMs de sessão usam rede em modo usuário.",
        "Las VM de sesión usan red en modo usuario."
    ),
    s!(
        "net.added",
        "Added network interface {mac}",
        "Добавлен интерфейс {mac}",
        "已添加网卡 {mac}",
        "Interface {mac} adicionada",
        "Interfaz {mac} añadida"
    ),
    s!(
        "net.add_failed",
        "Add interface failed: {error}",
        "Ошибка добавления интерфейса: {error}",
        "添加网卡失败：{error}",
        "Falha ao adicionar interface: {error}",
        "Error al añadir interfaz: {error}"
    ),
    s!(
        "net.removed",
        "Removed network interface {mac}",
        "Интерфейс {mac} удалён",
        "已移除网卡 {mac}",
        "Interface {mac} removida",
        "Interfaz {mac} quitada"
    ),
    s!(
        "net.remove_failed",
        "Remove interface failed: {error}",
        "Ошибка удаления интерфейса: {error}",
        "移除网卡失败：{error}",
        "Falha ao remover interface: {error}",
        "Error al quitar interfaz: {error}"
    ),
    s!(
        "net.none",
        "No network interfaces",
        "Нет сетевых интерфейсов",
        "没有网卡",
        "Nenhuma interface de rede",
        "No hay interfaces de red"
    ),
    s!(
        "net.select_hint",
        "Select an interface to remove it.",
        "Выберите интерфейс, чтобы удалить его.",
        "选择网卡后可移除。",
        "Selecione uma interface para removê-la.",
        "Selecciona una interfaz para quitarla."
    ),
    s!(
        "net.user",
        "user-mode",
        "user-mode",
        "用户模式",
        "modo usuário",
        "modo usuario"
    ),
    s!("net.bridge", "bridge", "мост", "网桥", "ponte", "puente"),
    s!("net.network", "network", "сеть", "网络", "rede", "red"),
    s!(
        "net.remove_title",
        "Remove interface {mac}?",
        "Удалить интерфейс {mac}?",
        "移除网卡 {mac}？",
        "Remover interface {mac}?",
        "¿Quitar interfaz {mac}?"
    ),
    s!(
        "net.remove_body",
        "The network interface will be removed from the VM configuration.",
        "Сетевой интерфейс будет удалён из конфигурации ВМ.",
        "该网卡将从虚拟机配置中移除。",
        "A interface será removida da configuração da VM.",
        "La interfaz se quitará de la configuración de la VM."
    ),
    s!(
        "set.language",
        "Language",
        "Язык",
        "语言",
        "Idioma",
        "Idioma"
    ),
    s!(
        "set.language_hint",
        "Changes apply immediately, no restart required.",
        "Изменения применяются сразу, перезапуск не нужен.",
        "更改立即生效，无需重启。",
        "As alterações são aplicadas imediatamente, sem reiniciar.",
        "Los cambios se aplican de inmediato, sin reiniciar."
    ),
    s!(
        "set.about",
        "About",
        "О программе",
        "关于",
        "Sobre",
        "Acerca de"
    ),
    s!(
        "set.about_text",
        "VMM-KVM is a QEMU/KVM manager built with Rust, gpui and libvirt. The PySide6 version remains the feature-complete reference.",
        "VMM-KVM — менеджер QEMU/KVM на Rust, gpui и libvirt. Версия на PySide6 остаётся эталонной по функциям.",
        "VMM-KVM 是基于 Rust、gpui 和 libvirt 的 QEMU/KVM 管理器。PySide6 版本仍是功能完整的参考实现。",
        "VMM-KVM é um gerenciador QEMU/KVM feito com Rust, gpui e libvirt. A versão PySide6 continua sendo a referência completa.",
        "VMM-KVM es un gestor QEMU/KVM creado con Rust, gpui y libvirt. La versión PySide6 sigue siendo la referencia completa."
    ),
    s!(
        "set.version",
        "Version",
        "Версия",
        "版本",
        "Versão",
        "Versión"
    ),
    s!(
        "set.license",
        "License",
        "Лицензия",
        "许可证",
        "Licença",
        "Licencia"
    ),
    s!(
        "cr.title",
        "Create a virtual machine",
        "Создание виртуальной машины",
        "创建虚拟机",
        "Criar uma máquina virtual",
        "Crear una máquina virtual"
    ),
    s!(
        "cr.subtitle",
        "Pick install media, confirm the detected OS and adjust hardware.",
        "Выберите установочный носитель, проверьте определённую ОС и настройте железо.",
        "选择安装介质，确认检测到的操作系统并调整硬件。",
        "Escolha a mídia de instalação, confirme o sistema detectado e ajuste o hardware.",
        "Elige el medio de instalación, confirma el sistema detectado y ajusta el hardware."
    ),
    s!(
        "cr.host",
        "host: {cpu} CPU · {ram}",
        "хост: {cpu} CPU · {ram}",
        "主机：{cpu} CPU · {ram}",
        "host: {cpu} CPU · {ram}",
        "host: {cpu} CPU · {ram}"
    ),
    s!(
        "cr.rescan",
        "Rescan ISOs",
        "Пересканировать ISO",
        "重新扫描 ISO",
        "Reexaminar ISOs",
        "Reescanear ISOs"
    ),
    s!(
        "cr.choose_iso",
        "Choose ISO…",
        "Выбрать ISO…",
        "选择 ISO…",
        "Escolher ISO…",
        "Elegir ISO…"
    ),
    s!(
        "cr.custom_iso",
        "Custom ISO: {name}",
        "Свой ISO: {name}",
        "自定义 ISO：{name}",
        "ISO personalizado: {name}",
        "ISO personalizado: {name}"
    ),
    s!(
        "cr.media",
        "INSTALLATION MEDIA",
        "УСТАНОВОЧНЫЙ НОСИТЕЛЬ",
        "安装介质",
        "MÍDIA DE INSTALAÇÃO",
        "MEDIO DE INSTALACIÓN"
    ),
    s!(
        "cr.no_media",
        "No installation media",
        "Без установочного носителя",
        "无安装介质",
        "Sem mídia de instalação",
        "Sin medio de instalación"
    ),
    s!(
        "cr.no_media_hint",
        "Create an empty VM and install later",
        "Создать пустую ВМ и установить позже",
        "创建空虚拟机，稍后安装",
        "Criar uma VM vazia e instalar depois",
        "Crear una VM vacía e instalar después"
    ),
    s!(
        "cr.iso_hint",
        "Place .iso files in ~/VirtualMachines/iso, then press Rescan ISOs.",
        "Положите .iso в ~/VirtualMachines/iso и нажмите «Пересканировать ISO».",
        "将 .iso 文件放入 ~/VirtualMachines/iso，然后点击重新扫描 ISO。",
        "Coloque arquivos .iso em ~/VirtualMachines/iso e clique em Reexaminar ISOs.",
        "Coloca archivos .iso en ~/VirtualMachines/iso y pulsa Reescanear ISOs."
    ),
    s!(
        "cr.os",
        "OPERATING SYSTEM",
        "ОПЕРАЦИОННАЯ СИСТЕМА",
        "操作系统",
        "SISTEMA OPERACIONAL",
        "SISTEMA OPERATIVO"
    ),
    s!("cr.name", "NAME", "ИМЯ", "名称", "NOME", "NOMBRE"),
    s!(
        "cr.name_field",
        "name:",
        "имя:",
        "名称：",
        "nome:",
        "nombre:"
    ),
    s!(
        "cr.name_auto",
        "auto-generated",
        "автоматически",
        "自动生成",
        "gerado automaticamente",
        "generado automáticamente"
    ),
    s!(
        "cr.name_manual",
        "custom",
        "вручную",
        "自定义",
        "personalizado",
        "personalizado"
    ),
    s!(
        "cr.name_reset",
        "Reset to auto",
        "Сбросить на авто",
        "恢复自动名称",
        "Voltar ao automático",
        "Volver a automático"
    ),
    s!(
        "cr.name_hint",
        "Letters, digits, dots, dashes and underscores are allowed.",
        "Разрешены буквы, цифры, точки, дефисы и подчёркивания.",
        "只能使用字母、数字、点、短横线和下划线。",
        "São permitidos letras, números, pontos, hífens e sublinhados.",
        "Se permiten letras, números, puntos, guiones y guiones bajos."
    ),
    s!(
        "cr.hardware",
        "HARDWARE",
        "ЖЕЛЕЗО",
        "硬件",
        "HARDWARE",
        "HARDWARE"
    ),
    s!(
        "cr.max",
        "max {max}",
        "макс. {max}",
        "最大 {max}",
        "máx. {max}",
        "máx. {max}"
    ),
    s!("cr.disk", "Disk", "Диск", "磁盘", "Disco", "Disco"),
    s!(
        "cr.disk_hint",
        "qcow2, grows on demand",
        "qcow2, растёт по мере заполнения",
        "qcow2，按需增长",
        "qcow2, cresce sob demanda",
        "qcow2, crece según demanda"
    ),
    s!(
        "cr.kvm",
        "KVM acceleration",
        "KVM-ускорение",
        "KVM 加速",
        "Aceleração KVM",
        "Aceleración KVM"
    ),
    s!(
        "cr.kvm_hint",
        "Recommended when /dev/kvm is available",
        "Рекомендуется, если доступен /dev/kvm",
        "当 /dev/kvm 可用时推荐",
        "Recomendado quando /dev/kvm está disponível",
        "Recomendado cuando /dev/kvm está disponible"
    ),
    s!(
        "cr.disk_path",
        "Disk: ~/VirtualMachines/vms/{name}/disk.qcow2 · {mode}",
        "Диск: ~/VirtualMachines/vms/{name}/disk.qcow2 · {mode}",
        "磁盘：~/VirtualMachines/vms/{name}/disk.qcow2 · {mode}",
        "Disco: ~/VirtualMachines/vms/{name}/disk.qcow2 · {mode}",
        "Disco: ~/VirtualMachines/vms/{name}/disk.qcow2 · {mode}"
    ),
    s!(
        "cr.boot_cd",
        "boot from CD-ROM",
        "загрузка с CD-ROM",
        "从 CD-ROM 启动",
        "iniciar do CD-ROM",
        "arrancar desde CD-ROM"
    ),
    s!(
        "cr.empty",
        "empty VM",
        "пустая ВМ",
        "空虚拟机",
        "VM vazia",
        "VM vacía"
    ),
    s!(
        "cr.cancel",
        "Cancel",
        "Отмена",
        "取消",
        "Cancelar",
        "Cancelar"
    ),
    s!(
        "cr.create",
        "Create virtual machine",
        "Создать виртуальную машину",
        "创建虚拟机",
        "Criar máquina virtual",
        "Crear máquina virtual"
    ),
    s!(
        "cr.name_taken",
        "Name is already taken",
        "Имя уже занято",
        "名称已被占用",
        "O nome já está em uso",
        "El nombre ya está en uso"
    ),
    s!(
        "cr.name_empty",
        "Enter a name for the virtual machine",
        "Введите имя виртуальной машины",
        "请输入虚拟机名称",
        "Digite um nome para a máquina virtual",
        "Introduce un nombre para la máquina virtual"
    ),
    s!(
        "cr.created",
        "Created {name}",
        "Создано: {name}",
        "已创建 {name}",
        "{name} criada",
        "{name} creada"
    ),
    s!(
        "cr.create_failed",
        "Create failed: {error}",
        "Ошибка создания: {error}",
        "创建失败：{error}",
        "Falha ao criar: {error}",
        "Error al crear: {error}"
    ),
    s!(
        "conn.title",
        "Cannot connect to libvirt",
        "Нет подключения к libvirt",
        "无法连接 libvirt",
        "Não foi possível conectar ao libvirt",
        "No se puede conectar a libvirt"
    ),
    s!(
        "conn.body",
        "Install libvirt, qemu and virt-viewer, then make sure the libvirt session daemon can start.",
        "Установите libvirt, qemu и virt-viewer и убедитесь, что сессионный демон libvirt запускается.",
        "请安装 libvirt、qemu 和 virt-viewer，并确保 libvirt 会话守护进程可以启动。",
        "Instale libvirt, qemu e virt-viewer e verifique se o daemon de sessão libvirt inicia.",
        "Instala libvirt, qemu y virt-viewer y asegúrate de que el demonio de sesión libvirt puede iniciarse."
    ),
    s!(
        "conn.retry",
        "Retry connection",
        "Повторить подключение",
        "重试连接",
        "Tentar novamente",
        "Reintentar conexión"
    ),
    s!(
        "err.not_connected",
        "Not connected to libvirt",
        "Нет подключения к libvirt",
        "未连接 libvirt",
        "Não conectado ao libvirt",
        "No conectado a libvirt"
    ),
    s!(
        "conn.connected",
        "Connected to {uri}",
        "Подключено к {uri}",
        "已连接到 {uri}",
        "Conectado a {uri}",
        "Conectado a {uri}"
    ),
    s!(
        "ph.storage_title",
        "Storage pools",
        "Хранилища",
        "存储池",
        "Pools de armazenamento",
        "Almacenamiento"
    ),
    s!(
        "ph.storage_body",
        "Manage libvirt storage pools and volumes here. The core build only manages VM disks in ~/VirtualMachines/vms.",
        "Здесь будут пулы и тома libvirt. Ядро пока управляет только дисками ВМ в ~/VirtualMachines/vms.",
        "在此管理 libvirt 存储池和卷。当前核心版本仅管理 ~/VirtualMachines/vms 中的虚拟机磁盘。",
        "Gerencie pools e volumes libvirt aqui. O núcleo atual gerencia apenas discos de VM em ~/VirtualMachines/vms.",
        "Gestiona pools y volúmenes libvirt aquí. El núcleo actual solo gestiona discos de VM en ~/VirtualMachines/vms."
    ),
    s!(
        "ph.networks_title",
        "Virtual networks",
        "Виртуальные сети",
        "虚拟网络",
        "Redes virtuais",
        "Redes virtuales"
    ),
    s!(
        "ph.networks_body",
        "Browse libvirt networks and bridges here. Session VMs use user-mode networking for zero configuration.",
        "Здесь будут сети и мосты libvirt. Сессионные ВМ используют user-mode сеть без настройки.",
        "在此浏览 libvirt 网络和网桥。会话虚拟机使用用户模式网络，无需配置。",
        "Veja redes e pontes libvirt aqui. VMs de sessão usam rede em modo usuário sem configuração.",
        "Consulta redes y puentes libvirt aquí. Las VM de sesión usan red en modo usuario sin configuración."
    ),
    s!(
        "ph.ssh_title",
        "SSH hosts",
        "SSH-хосты",
        "SSH 主机",
        "Hosts SSH",
        "Hosts SSH"
    ),
    s!(
        "ph.ssh_body",
        "Connect to remote libvirt daemons over SSH. This screen is planned for the next build.",
        "Подключение к удалённым демонам libvirt по SSH. Экран запланирован на следующий билд.",
        "通过 SSH 连接远程 libvirt 守护进程。此界面计划在下个版本推出。",
        "Conecte-se a daemons libvirt remotos via SSH. Esta tela está planejada para a próxima versão.",
        "Conéctate a demonios libvirt remotos por SSH. Esta pantalla está prevista para la próxima versión."
    ),
    s!(
        "ph.back",
        "Back to virtual machines",
        "Назад к виртуальным машинам",
        "返回虚拟机",
        "Voltar às máquinas virtuais",
        "Volver a las máquinas virtuales"
    ),
    s!("status.ready", "Ready", "Готово", "就绪", "Pronto", "Listo"),
    s!(
        "status.refreshed",
        "Refreshed",
        "Обновлено",
        "已刷新",
        "Atualizado",
        "Actualizado"
    ),
    s!(
        "status.deleted",
        "Deleted {name}",
        "Удалено: {name}",
        "已删除 {name}",
        "{name} excluída",
        "{name} eliminada"
    ),
    s!(
        "status.delete_failed",
        "Delete failed: {error}",
        "Ошибка удаления: {error}",
        "删除失败：{error}",
        "Falha ao excluir: {error}",
        "Error al eliminar: {error}"
    ),
    s!(
        "status.stop_before_delete",
        "Stop the virtual machine before deleting it",
        "Остановите виртуальную машину перед удалением",
        "删除前请先停止虚拟机",
        "Pare a máquina virtual antes de excluí-la",
        "Detén la máquina virtual antes de eliminarla"
    ),
    s!(
        "status.delete_title",
        "Delete {name}?",
        "Удалить {name}?",
        "删除 {name}？",
        "Excluir {name}?",
        "¿Eliminar {name}?"
    ),
    s!(
        "status.delete_body",
        "The VM definition and NVRAM will be removed. Disk files stay on disk.",
        "Определение ВМ и NVRAM будут удалены. Файлы дисков останутся на диске.",
        "虚拟机的定义和 NVRAM 将被移除，磁盘文件仍保留。",
        "A definição da VM e a NVRAM serão removidas. Os arquivos de disco permanecem.",
        "Se quitarán la definición de la VM y la NVRAM. Los archivos de disco permanecen."
    ),
    s!(
        "status.starting",
        "Starting {name}",
        "Запуск {name}",
        "正在启动 {name}",
        "Iniciando {name}",
        "Iniciando {name}"
    ),
    s!(
        "status.shutdown_requested",
        "Shutdown requested for {name}",
        "Запрошено завершение работы {name}",
        "已请求关闭 {name}",
        "Desligamento solicitado para {name}",
        "Apagado solicitado para {name}"
    ),
    s!(
        "status.powered_off",
        "{name} powered off",
        "{name} выключена",
        "{name} 已关机",
        "{name} desligada",
        "{name} apagada"
    ),
    s!(
        "status.rebooting",
        "Rebooting {name}",
        "Перезагрузка {name}",
        "正在重启 {name}",
        "Reiniciando {name}",
        "Reiniciando {name}"
    ),
    s!(
        "status.paused",
        "{name} paused",
        "{name} на паузе",
        "{name} 已暂停",
        "{name} pausada",
        "{name} pausada"
    ),
    s!(
        "status.resumed",
        "{name} resumed",
        "{name} продолжена",
        "{name} 已继续",
        "{name} retomada",
        "{name} reanudada"
    ),
    s!(
        "status.console",
        "Console launched",
        "Консоль запущена",
        "控制台已启动",
        "Console iniciada",
        "Consola iniciada"
    ),
    s!(
        "status.hw_updated",
        "Hardware updated",
        "Железо обновлено",
        "硬件已更新",
        "Hardware atualizado",
        "Hardware actualizado"
    ),
    s!(
        "status.hw_deferred",
        "Saved. RAM and vCPU changes take effect after the next start.",
        "Сохранено. Изменения RAM и vCPU применятся после следующего запуска.",
        "已保存。RAM 和 vCPU 更改将在下次启动后生效。",
        "Salvo. Alterações de RAM e vCPU entram em vigor na próxima inicialização.",
        "Guardado. Los cambios de RAM y vCPU se aplican en el próximo inicio."
    ),
    s!(
        "status.hw_failed",
        "Apply failed: {error}",
        "Ошибка применения: {error}",
        "应用失败：{error}",
        "Falha ao aplicar: {error}",
        "Error al aplicar: {error}"
    ),
    s!(
        "status.iso_found",
        "{n} ISO file(s) found in ~/VirtualMachines/iso",
        "Найдено ISO: {n} в ~/VirtualMachines/iso",
        "在 ~/VirtualMachines/iso 中找到 {n} 个 ISO 文件",
        "{n} arquivo(s) ISO encontrados em ~/VirtualMachines/iso",
        "{n} archivo(s) ISO encontrados en ~/VirtualMachines/iso"
    ),
    s!(
        "status.no_iso",
        "No ISO files found in ~/VirtualMachines/iso",
        "В ~/VirtualMachines/iso нет ISO-файлов",
        "在 ~/VirtualMachines/iso 中未找到 ISO 文件",
        "Nenhum arquivo ISO encontrado em ~/VirtualMachines/iso",
        "No se encontraron archivos ISO en ~/VirtualMachines/iso"
    ),
    s!(
        "status.lang_changed",
        "Language changed to {lang}",
        "Язык изменён на {lang}",
        "语言已切换为 {lang}",
        "Idioma alterado para {lang}",
        "Idioma cambiado a {lang}"
    ),
    s!(
        "status.selected_iso",
        "Selected ISO: {name}",
        "Выбран ISO: {name}",
        "已选择 ISO：{name}",
        "ISO selecionado: {name}",
        "ISO seleccionado: {name}"
    ),
    s!(
        "status.iso_pick_failed",
        "Could not open the file picker",
        "Не удалось открыть выбор файла",
        "无法打开文件选择器",
        "Não foi possível abrir o seletor de arquivos",
        "No se pudo abrir el selector de archivos"
    ),
    s!(
        "status.settings",
        "Settings",
        "Настройки",
        "设置",
        "Configurações",
        "Ajustes"
    ),
    s!(
        "settings.current",
        "Current: {lang}",
        "Текущий: {lang}",
        "当前：{lang}",
        "Atual: {lang}",
        "Actual: {lang}"
    ),
];

pub fn tr(lang: Lang, key: &str) -> &'static str {
    STRINGS
        .iter()
        .find(|entry| entry.key == key)
        .map(|entry| entry.get(lang))
        .unwrap_or("?")
}

pub fn tf(lang: Lang, key: &str, args: &[(&str, String)]) -> String {
    let mut out = tr(lang, key).to_string();
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

pub fn short(lang: Lang) -> &'static str {
    lang.short()
}

pub fn next(lang: Lang) -> Lang {
    lang.next()
}

pub fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("vmm-kvm")
}

pub fn load_lang() -> Lang {
    if let Ok(code) = fs::read_to_string(config_dir().join("language")) {
        if let Some(lang) = Lang::from_code(&code) {
            return lang;
        }
    }
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(var) {
            let prefix = value.split(['_', '.']).next().unwrap_or("").to_lowercase();
            if let Some(lang) = Lang::from_code(&prefix) {
                return lang;
            }
        }
    }
    Lang::En
}

pub fn save_lang(lang: Lang) {
    let dir = config_dir();
    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(dir.join("language"), lang.code());
}
