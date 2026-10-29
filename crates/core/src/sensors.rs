//! CPU temperature and fan speed from the System Management Controller (Intel Macs).
//! Reading the SMC needs no root (writing does, and we never write). The interface is
//! undocumented but long-stable; anything unexpected just means "no reading".
//! Apple Silicon reports through a different interface and isn't covered yet: `read`
//! returns None there and the menu bar hides the tile.

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Sensors {
    /// Degrees Celsius.
    pub cpu_celsius: Option<f32>,
    /// Fastest fan right now, and that fan's maximum.
    pub fan_rpm: Option<u32>,
    pub fan_max_rpm: Option<u32>,
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
mod smc {
    use std::ffi::{c_char, c_void};

    type KernReturn = i32;
    type IoObject = u32;

    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        fn IOServiceMatching(name: *const c_char) -> *mut c_void;
        fn IOServiceGetMatchingService(main_port: u32, matching: *mut c_void) -> IoObject;
        fn IOServiceOpen(service: IoObject, owning_task: u32, kind: u32, connect: *mut IoObject) -> KernReturn;
        fn IOServiceClose(connect: IoObject) -> KernReturn;
        fn IOObjectRelease(object: IoObject) -> KernReturn;
        fn IOConnectCallStructMethod(
            connection: IoObject,
            selector: u32,
            input: *const c_void,
            input_size: usize,
            output: *mut c_void,
            output_size: *mut usize,
        ) -> KernReturn;
    }
    extern "C" {
        static mach_task_self_: u32;
    }

    // The AppleSMC user client's call layout (as used by every SMC reader since 2006).
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct Vers {
        major: u8,
        minor: u8,
        build: u8,
        reserved: u8,
        release: u16,
    }
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct PLimit {
        version: u16,
        length: u16,
        cpu: u32,
        gpu: u32,
        mem: u32,
    }
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct KeyInfo {
        data_size: u32,
        data_type: u32,
        attributes: u8,
    }
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct KeyData {
        key: u32,
        vers: Vers,
        p_limit: PLimit,
        info: KeyInfo,
        result: u8,
        status: u8,
        data8: u8,
        data32: u32,
        bytes: [u8; 32],
    }
    const _: () = assert!(std::mem::size_of::<KeyData>() == 80);

    const KERNEL_INDEX_SMC: u32 = 2;
    const CMD_READ_BYTES: u8 = 5;
    const CMD_READ_KEYINFO: u8 = 9;

    fn fourcc(s: &str) -> u32 {
        s.bytes().fold(0, |acc, b| (acc << 8) | b as u32)
    }

    pub struct Smc(IoObject);

    impl Smc {
        pub fn open() -> Option<Smc> {
            unsafe {
                let service = IOServiceGetMatchingService(0, IOServiceMatching(c"AppleSMC".as_ptr()));
                if service == 0 {
                    return None;
                }
                let mut conn = 0;
                let r = IOServiceOpen(service, mach_task_self_, 0, &mut conn);
                IOObjectRelease(service);
                (r == 0).then_some(Smc(conn))
            }
        }

        fn call(&self, input: &KeyData) -> Option<KeyData> {
            let mut out = KeyData::default();
            let mut size = std::mem::size_of::<KeyData>();
            let r = unsafe {
                IOConnectCallStructMethod(
                    self.0,
                    KERNEL_INDEX_SMC,
                    (input as *const KeyData).cast(),
                    std::mem::size_of::<KeyData>(),
                    (&mut out as *mut KeyData).cast(),
                    &mut size,
                )
            };
            (r == 0 && out.result == 0).then_some(out)
        }

        /// The key's raw bytes and its type (e.g. "sp78", "fpe2", "flt ").
        pub fn read(&self, key: &str) -> Option<(Vec<u8>, String)> {
            let mut q = KeyData { key: fourcc(key), data8: CMD_READ_KEYINFO, ..Default::default() };
            let info = self.call(&q)?.info;
            if info.data_size == 0 || info.data_size > 32 {
                return None;
            }
            q.info = info;
            q.data8 = CMD_READ_BYTES;
            let out = self.call(&q)?;
            let kind = String::from_utf8_lossy(&info.data_type.to_be_bytes()).into_owned();
            Some((out.bytes[..info.data_size as usize].to_vec(), kind))
        }

        /// A numeric key decoded from the SMC's fixed-point and float types.
        pub fn number(&self, key: &str) -> Option<f32> {
            let (b, kind) = self.read(key)?;
            let be16 = || b.get(..2).map(|x| u16::from_be_bytes([x[0], x[1]]));
            match kind.as_str() {
                "sp78" => be16().map(|v| v as i16 as f32 / 256.0),
                "fpe2" => be16().map(|v| v as f32 / 4.0),
                "flt " => b.get(..4).map(|x| f32::from_le_bytes([x[0], x[1], x[2], x[3]])),
                "ui8 " => b.first().map(|v| *v as f32),
                "ui16" => be16().map(|v| v as f32),
                _ => None,
            }
        }
    }

    impl Drop for Smc {
        fn drop(&mut self) {
            unsafe { IOServiceClose(self.0) };
        }
    }
}

/// CPU temperature keys, most representative first: package/die, then proximity.
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const CPU_KEYS: [&str; 5] = ["TC0F", "TC0D", "TC0E", "TCXC", "TC0P"];

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
pub fn read() -> Option<Sensors> {
    let smc = smc::Smc::open()?;
    let plausible = |c: &f32| (5.0..125.0).contains(c);
    let cpu_celsius = CPU_KEYS.iter().find_map(|k| smc.number(k).filter(plausible));

    let fans = smc.number("FNum").map_or(0, |n| n as u32).min(4);
    let (mut fan_rpm, mut fan_max_rpm) = (None, None);
    for i in 0..fans {
        let Some(rpm) = smc.number(&format!("F{i}Ac")).filter(|r| (0.0..20_000.0).contains(r)) else { continue };
        if fan_rpm.is_none_or(|best| rpm as u32 > best) {
            fan_rpm = Some(rpm as u32);
            fan_max_rpm = smc.number(&format!("F{i}Mx")).filter(|m| *m > 0.0).map(|m| m as u32);
        }
    }
    (cpu_celsius.is_some() || fan_rpm.is_some()).then_some(Sensors { cpu_celsius, fan_rpm, fan_max_rpm })
}

/// Every CPU key and its value, for checking which sensor a Mac has.
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
pub fn cpu_keys() -> Vec<(&'static str, Option<f32>)> {
    let Some(smc) = smc::Smc::open() else { return vec![] };
    CPU_KEYS.iter().map(|k| (*k, smc.number(k))).collect()
}

#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
pub fn read() -> Option<Sensors> {
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn readings_are_plausible_when_present() {
        // Virtual machines (CI) have no SMC sensors: None is fine there.
        if let Some(s) = super::read() {
            if let Some(c) = s.cpu_celsius {
                assert!((5.0..125.0).contains(&c), "{c}");
            }
            if let (Some(r), Some(m)) = (s.fan_rpm, s.fan_max_rpm) {
                assert!(r <= m + 500, "{r} of {m}");
            }
        }
    }
}
