//! Anthropic API key resolution.
//!
//! Order: `ANTHROPIC_API_KEY` environment variable, then a DPAPI-encrypted
//! file at `%APPDATA%\RustyTerminal\api_key.bin`. The key is never
//! hardcoded, never logged, never stored in the SQLite DB.

use std::path::PathBuf;

fn key_file_path() -> PathBuf {
    let mut dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("RustyTerminal");
    let _ = std::fs::create_dir_all(&dir);
    dir.push("api_key.bin");
    dir
}

/// Resolve the API key, if configured.
pub fn resolve() -> Option<String> {
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        let key = key.trim().to_string();
        if !key.is_empty() {
            return Some(key);
        }
    }
    read_encrypted_key()
}

/// True if a key is available from either source.
pub fn is_configured() -> bool {
    resolve().is_some()
}

/// Store the key DPAPI-encrypted (current user scope) on disk.
pub fn store_encrypted_key(key: &str) -> std::io::Result<()> {
    let encrypted = dpapi::protect(key.trim().as_bytes())?;
    std::fs::write(key_file_path(), encrypted)
}

pub fn delete_stored_key() -> std::io::Result<()> {
    let path = key_file_path();
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

fn read_encrypted_key() -> Option<String> {
    let bytes = std::fs::read(key_file_path()).ok()?;
    let decrypted = dpapi::unprotect(&bytes).ok()?;
    let key = String::from_utf8(decrypted).ok()?.trim().to_string();
    (!key.is_empty()).then_some(key)
}

/// Minimal DPAPI wrapper (CryptProtectData / CryptUnprotectData,
/// current-user scope).
#[cfg(windows)]
mod dpapi {
    use std::io;

    #[repr(C)]
    struct DataBlob {
        cb_data: u32,
        pb_data: *mut u8,
    }

    #[link(name = "crypt32")]
    unsafe extern "system" {
        fn CryptProtectData(
            data_in: *const DataBlob,
            descr: *const u16,
            entropy: *const DataBlob,
            reserved: *mut core::ffi::c_void,
            prompt: *mut core::ffi::c_void,
            flags: u32,
            data_out: *mut DataBlob,
        ) -> i32;
        fn CryptUnprotectData(
            data_in: *const DataBlob,
            descr: *mut *mut u16,
            entropy: *const DataBlob,
            reserved: *mut core::ffi::c_void,
            prompt: *mut core::ffi::c_void,
            flags: u32,
            data_out: *mut DataBlob,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LocalFree(mem: *mut core::ffi::c_void)
            -> *mut core::ffi::c_void;
    }

    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;

    pub fn protect(data: &[u8]) -> io::Result<Vec<u8>> {
        let input = DataBlob {
            cb_data: data.len() as u32,
            pb_data: data.as_ptr() as *mut u8,
        };
        let mut output = DataBlob {
            cb_data: 0,
            pb_data: std::ptr::null_mut(),
        };
        let ok = unsafe {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 {
            return Err(io::Error::other("CryptProtectData failed"));
        }
        let result = unsafe {
            std::slice::from_raw_parts(output.pb_data, output.cb_data as usize)
                .to_vec()
        };
        unsafe { LocalFree(output.pb_data as *mut _) };
        Ok(result)
    }

    pub fn unprotect(data: &[u8]) -> io::Result<Vec<u8>> {
        let input = DataBlob {
            cb_data: data.len() as u32,
            pb_data: data.as_ptr() as *mut u8,
        };
        let mut output = DataBlob {
            cb_data: 0,
            pb_data: std::ptr::null_mut(),
        };
        let ok = unsafe {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 {
            return Err(io::Error::other("CryptUnprotectData failed"));
        }
        let result = unsafe {
            std::slice::from_raw_parts(output.pb_data, output.cb_data as usize)
                .to_vec()
        };
        unsafe { LocalFree(output.pb_data as *mut _) };
        Ok(result)
    }
}

#[cfg(not(windows))]
mod dpapi {
    use std::io;
    pub fn protect(_data: &[u8]) -> io::Result<Vec<u8>> {
        Err(io::Error::other("DPAPI is Windows-only"))
    }
    pub fn unprotect(_data: &[u8]) -> io::Result<Vec<u8>> {
        Err(io::Error::other("DPAPI is Windows-only"))
    }
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn dpapi_roundtrip() {
        let secret: &[u8] = b"sk-ant-test-key";
        let encrypted = super::dpapi::protect(secret).unwrap();
        assert_ne!(encrypted.as_slice(), secret, "must not be plaintext");
        let decrypted = super::dpapi::unprotect(&encrypted).unwrap();
        assert_eq!(decrypted.as_slice(), secret);
    }
}
