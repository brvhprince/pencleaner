//! Small wrappers over the Windows registry (current user and local machine).

use windows_sys::Win32::System::Registry::{
    RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, REG_SZ, RRF_RT_REG_BINARY,
    RRF_RT_REG_SZ,
};

/// Which hive: the user's own settings or the machine's.
#[derive(Clone, Copy)]
pub enum Root {
    CurrentUser,
    LocalMachine,
}

impl Root {
    fn hkey(self) -> HKEY {
        match self {
            Root::CurrentUser => HKEY_CURRENT_USER,
            Root::LocalMachine => HKEY_LOCAL_MACHINE,
        }
    }
}

pub const CURRENT_USER: Root = Root::CurrentUser;
pub const LOCAL_MACHINE: Root = Root::LocalMachine;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn get_raw(root: Root, key: &str, value: &str, flags: u32) -> Option<Vec<u8>> {
    let (k, v) = (wide(key), wide(value));
    let mut len: u32 = 0;
    let r = unsafe { RegGetValueW(root.hkey(), k.as_ptr(), v.as_ptr(), flags, std::ptr::null_mut(), std::ptr::null_mut(), &mut len) };
    if r != 0 || len == 0 {
        return None;
    }
    let mut buf = vec![0u8; len as usize];
    let r = unsafe { RegGetValueW(root.hkey(), k.as_ptr(), v.as_ptr(), flags, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut len) };
    (r == 0).then(|| {
        buf.truncate(len as usize);
        buf
    })
}

pub fn get_string(root: Root, key: &str, value: &str) -> Option<String> {
    let raw = get_raw(root, key, value, RRF_RT_REG_SZ)?;
    let units: Vec<u16> = raw.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).take_while(|u| *u != 0).collect();
    Some(String::from_utf16_lossy(&units))
}

pub fn get_binary(root: Root, key: &str, value: &str) -> Option<Vec<u8>> {
    get_raw(root, key, value, RRF_RT_REG_BINARY)
}

pub fn set_string(root: Root, key: &str, value: &str, data: &str) -> Result<(), String> {
    let (k, v, d) = (wide(key), wide(value), wide(data));
    let r = unsafe { RegSetKeyValueW(root.hkey(), k.as_ptr(), v.as_ptr(), REG_SZ, d.as_ptr().cast(), (d.len() * 2) as u32) };
    if r == 0 { Ok(()) } else { Err(std::io::Error::from_raw_os_error(r as i32).to_string()) }
}

/// Removing a value that isn't there counts as done.
pub fn delete(root: Root, key: &str, value: &str) -> Result<(), String> {
    const ERROR_FILE_NOT_FOUND: u32 = 2;
    let (k, v) = (wide(key), wide(value));
    let r = unsafe { RegDeleteKeyValueW(root.hkey(), k.as_ptr(), v.as_ptr()) };
    if r == 0 || r == ERROR_FILE_NOT_FOUND { Ok(()) } else { Err(std::io::Error::from_raw_os_error(r as i32).to_string()) }
}
