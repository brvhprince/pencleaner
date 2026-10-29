//! Windows readings for the tray popover, from Windows' own counters; no admin rights.
//! CPU and network are rates, so a `Sampler` keeps the previous reading.

use super::{health, Battery, Vitals};
use crate::volumes;
use crate::windows::registry;
use parking_lot::Mutex;
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::FILETIME;
use windows_sys::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};
use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows_sys::Win32::System::SystemInformation::{GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows_sys::Win32::System::Threading::GetSystemTimes;

/// Battery cycle count and health %.
type Wear = (Option<u32>, Option<u8>);

#[derive(Default)]
struct State {
    /// (busy, total) 100 ns ticks.
    cpu: Option<(u64, u64)>,
    net: Option<(u64, u64, Instant)>,
    model: Option<String>,
    trash: Option<(u64, Instant)>,
    wear: Option<(Wear, Instant)>,
}

#[derive(Default)]
pub struct Sampler(Mutex<State>);

fn ticks(t: FILETIME) -> u64 {
    ((t.dwHighDateTime as u64) << 32) | t.dwLowDateTime as u64
}

/// Cumulative (busy, total) CPU time across all cores. Kernel time includes idle.
fn cpu_times() -> Option<(u64, u64)> {
    let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut idle, mut kernel, mut user) = (zero, zero, zero);
    if unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } == 0 {
        return None;
    }
    let total = ticks(kernel) + ticks(user);
    Some((total.saturating_sub(ticks(idle)), total))
}

/// Bytes received and sent on real network adapters (not loopback or virtual ones).
fn net_bytes() -> (u64, u64) {
    const IF_TYPE_SOFTWARE_LOOPBACK: u32 = 24;
    const IF_OPER_STATUS_UP: i32 = 1;
    let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
    if unsafe { GetIfTable2(&mut table) } != 0 || table.is_null() {
        return (0, 0);
    }
    let (mut down, mut up) = (0, 0);
    unsafe {
        let n = (*table).NumEntries as usize;
        let rows = std::slice::from_raw_parts((*table).Table.as_ptr(), n);
        for r in rows {
            // Bit 0 of the flags: a hardware interface (Wi-Fi, Ethernet), not a filter or tunnel.
            let hardware = r.InterfaceAndOperStatusFlags._bitfield & 1 != 0;
            if hardware && r.Type != IF_TYPE_SOFTWARE_LOOPBACK && r.OperStatus == IF_OPER_STATUS_UP {
                down += r.InOctets;
                up += r.OutOctets;
            }
        }
        FreeMibTable(table.cast());
    }
    (down, up)
}

fn battery() -> Option<Battery> {
    let mut s: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
    if unsafe { GetSystemPowerStatus(&mut s) } == 0 {
        return None;
    }
    const NO_BATTERY: u8 = 128;
    const UNKNOWN: u8 = 255;
    const CHARGING: u8 = 8;
    if s.BatteryFlag == UNKNOWN || s.BatteryFlag & NO_BATTERY != 0 || s.BatteryLifePercent > 100 {
        return None;
    }
    let plugged = s.ACLineStatus == 1;
    let state = if s.BatteryFlag & CHARGING != 0 {
        "charging"
    } else if plugged && s.BatteryLifePercent >= 99 {
        "charged"
    } else if plugged {
        "plugged in"
    } else {
        "discharging"
    };
    let remaining = (!plugged && s.BatteryLifeTime != u32::MAX).then(|| format!("{}:{:02}", s.BatteryLifeTime / 3600, s.BatteryLifeTime / 60 % 60));
    Some(Battery { percent: s.BatteryLifePercent, state: state.into(), remaining, cycles: None, health: None })
}

/// Cycle count and health % (full-charge capacity against the design), from the battery
/// driver; normal users may ask it.
fn battery_wear() -> Wear {
    use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
        SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW, DIGCF_DEVICEINTERFACE,
        DIGCF_PRESENT, GUID_DEVCLASS_BATTERY, SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
    };
    use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_READ, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
    use windows_sys::Win32::System::IO::DeviceIoControl;
    use windows_sys::Win32::System::Power::{
        BatteryInformation, BATTERY_INFORMATION, BATTERY_QUERY_INFORMATION, IOCTL_BATTERY_QUERY_INFORMATION, IOCTL_BATTERY_QUERY_TAG,
    };
    unsafe {
        let set = SetupDiGetClassDevsW(&GUID_DEVCLASS_BATTERY, std::ptr::null(), std::ptr::null_mut(), DIGCF_PRESENT | DIGCF_DEVICEINTERFACE);
        if set == 0 || set == -1 {
            return (None, None);
        }
        let mut found = (None, None);
        let mut iface: SP_DEVICE_INTERFACE_DATA = std::mem::zeroed();
        iface.cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;
        // The first battery is the internal one on a laptop.
        if SetupDiEnumDeviceInterfaces(set, std::ptr::null(), &GUID_DEVCLASS_BATTERY, 0, &mut iface) != 0 {
            let mut need = 0u32;
            SetupDiGetDeviceInterfaceDetailW(set, &iface, std::ptr::null_mut(), 0, &mut need, std::ptr::null_mut());
            // u32-aligned buffer; the path starts after the cbSize field.
            let mut buf = vec![0u32; (need as usize).div_ceil(4).max(2)];
            let detail = buf.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
            (*detail).cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
            if SetupDiGetDeviceInterfaceDetailW(set, &iface, detail, need, std::ptr::null_mut(), std::ptr::null_mut()) != 0 {
                let path = std::ptr::addr_of!((*detail).DevicePath) as *const u16;
                let h = CreateFileW(path, GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE, std::ptr::null(), OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, std::ptr::null_mut());
                if h != INVALID_HANDLE_VALUE {
                    let (mut wait, mut tag, mut got) = (0u32, 0u32, 0u32);
                    let tagged = DeviceIoControl(
                        h,
                        IOCTL_BATTERY_QUERY_TAG,
                        (&mut wait as *mut u32).cast(),
                        4,
                        (&mut tag as *mut u32).cast(),
                        4,
                        &mut got,
                        std::ptr::null_mut(),
                    ) != 0;
                    if tagged && tag != 0 {
                        let q = BATTERY_QUERY_INFORMATION { BatteryTag: tag, InformationLevel: BatteryInformation, AtRate: 0 };
                        let mut info: BATTERY_INFORMATION = std::mem::zeroed();
                        let ok = DeviceIoControl(
                            h,
                            IOCTL_BATTERY_QUERY_INFORMATION,
                            (&q as *const BATTERY_QUERY_INFORMATION).cast(),
                            std::mem::size_of::<BATTERY_QUERY_INFORMATION>() as u32,
                            (&mut info as *mut BATTERY_INFORMATION).cast(),
                            std::mem::size_of::<BATTERY_INFORMATION>() as u32,
                            &mut got,
                            std::ptr::null_mut(),
                        ) != 0;
                        if ok {
                            let health = (info.DesignedCapacity > 0 && info.FullChargedCapacity > 0)
                                .then(|| ((info.FullChargedCapacity as u64 * 100 / info.DesignedCapacity as u64).min(100)) as u8);
                            let cycles = (info.CycleCount > 0).then_some(info.CycleCount);
                            found = (cycles, health);
                        }
                    }
                    CloseHandle(h);
                }
            }
        }
        SetupDiDestroyDeviceInfoList(set);
        found
    }
}

/// The maker's model name, e.g. "XPS 15 9520"; "PC" when the firmware doesn't say.
fn model_name() -> String {
    let bios = r"HARDWARE\DESCRIPTION\System\BIOS";
    registry::get_string(registry::LOCAL_MACHINE, bios, "SystemProductName")
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("System Product Name") && !s.contains("To be filled"))
        .unwrap_or_else(|| "PC".into())
}

impl Sampler {
    pub fn read(&self) -> Vitals {
        let mut st = self.0.lock();

        let cpu_load = match (cpu_times(), st.cpu) {
            (Some((busy, total)), Some((b0, t0))) => {
                st.cpu = Some((busy, total));
                (busy.saturating_sub(b0) * 100).checked_div(total.saturating_sub(t0)).unwrap_or(0).min(100) as u8
            }
            (now, _) => {
                st.cpu = now;
                0
            }
        };

        let (down, up) = net_bytes();
        let t = Instant::now();
        let (net_down, net_up) = match st.net {
            Some((d0, u0, t0)) => {
                let secs = t.duration_since(t0).as_secs_f64().max(0.001);
                ((down.saturating_sub(d0) as f64 / secs) as u64, (up.saturating_sub(u0) as f64 / secs) as u64)
            }
            None => (0, 0),
        };
        st.net = Some((down, up, t));

        let model = st.model.get_or_insert_with(model_name).clone();
        let wear = match st.wear {
            Some((w, at)) if at.elapsed() < Duration::from_secs(300) => w,
            _ => {
                let w = battery_wear();
                st.wear = Some((w, Instant::now()));
                w
            }
        };
        let trash = match st.trash {
            Some((b, at)) if at.elapsed() < Duration::from_secs(30) => b,
            _ => {
                let b = crate::windows::recycle_bin::size().0;
                st.trash = Some((b, Instant::now()));
                b
            }
        };
        drop(st);

        let mut mem: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
        mem.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        let ok = unsafe { GlobalMemoryStatusEx(&mut mem) } != 0;
        let load = if ok { mem.dwMemoryLoad.min(100) as u8 } else { 0 };
        // Committed memory beyond physical RAM lives in the page file.
        let committed = mem.ullTotalPageFile.saturating_sub(mem.ullAvailPageFile);
        let in_ram = mem.ullTotalPhys.saturating_sub(mem.ullAvailPhys);
        let level = if load >= 95 { "critical" } else if load >= 85 { "warning" } else { "normal" };

        let disk = volumes::list().into_iter().find(|v| v.is_root);
        let (disk_name, disk_free, disk_total) = disk.map_or(("PC".into(), 0, 0), |d| (d.name, d.free, d.total));

        Vitals {
            model,
            health: health(disk_free, disk_total, level, 0.0),
            disk_name,
            disk_free,
            disk_total,
            memory_pressure: load,
            memory_level: level.into(),
            swap_used: committed.saturating_sub(in_ram),
            cpu_load,
            battery: battery().map(|b| Battery { cycles: wear.0, health: wear.1, ..b }),
            net_up,
            net_down,
            trash,
            uptime_secs: unsafe { GetTickCount64() } / 1000,
            sensors: None,
        }
    }
}
