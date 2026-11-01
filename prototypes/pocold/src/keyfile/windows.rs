//! Windows keys are DPAPI user-bound ciphertext, never Unix plaintext with ignored modes.
//! Back up the Windows user profile as well as this file; it is not a portable secret export.
use super::*;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
};
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    },
    Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY,
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_OPEN_REPARSE_POINT,
    },
};
const MAGIC: &[u8] = b"POCOL-DPAPI-USER-V1\0";
const MAX: usize = 4096;

fn transform(input: &mut [u8], encrypt: bool) -> Result<Zeroizing<Vec<u8>>, String> {
    let blob = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_mut_ptr(),
    };
    let mut output: CRYPT_INTEGER_BLOB = unsafe { std::mem::zeroed() };
    // Deliberately omit CRYPTPROTECT_LOCAL_MACHINE. No UI, password or secret on the command line.
    let ok = unsafe {
        if encrypt {
            CryptProtectData(
                &blob,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &blob,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if ok == 0 {
        return Err("DPAPI operation failed for this Windows user".into());
    }
    let result = if output.cbData as usize > MAX || output.pbData.is_null() {
        Err("invalid DPAPI output".into())
    } else {
        Ok(Zeroizing::new(
            unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec(),
        ))
    };
    if !output.pbData.is_null() {
        // Volatile zeroing before returning the DPAPI allocation to Windows.
        for i in 0..output.cbData as usize {
            unsafe {
                output.pbData.add(i).write_volatile(0);
            }
        }
        unsafe {
            LocalFree(output.pbData.cast());
        }
    }
    result
}
fn inspect(file: &File) -> Result<(), String> {
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    if unsafe { GetFileInformationByHandle(file.as_raw_handle().cast(), &mut info) } == 0
        || info.dwFileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT) != 0
        || info.nNumberOfLinks != 1
    {
        return Err("key must be a single-link regular file, never a reparse point".into());
    }
    Ok(())
}
pub fn load(path: &Path) -> Result<Key, String> {
    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|_| "cannot open Windows key file")?;
    inspect(&file)?;
    let mut encoded = Zeroizing::new(Vec::new());
    Read::by_ref(&mut file)
        .take((MAX + 1) as u64)
        .read_to_end(&mut encoded)
        .map_err(|_| "cannot read Windows key")?;
    if encoded.len() > MAX || !encoded.starts_with(MAGIC) {
        return Err("expected a user-bound PoCol DPAPI key file".into());
    }
    let plain = transform(&mut encoded[MAGIC.len()..], false)?;
    let secret: &[u8; 32] = plain
        .as_slice()
        .try_into()
        .map_err(|_| "invalid decrypted key size")?;
    Key::from_secret(secret).map_err(|_| "invalid secp256k1 secret scalar".into())
}
pub fn generate(path: &Path) -> Result<Key, String> {
    let mut secret = Zeroizing::new([0u8; 32]);
    let key = loop {
        getrandom::getrandom(secret.as_mut()).map_err(|_| "OS randomness unavailable")?;
        if let Ok(key) = Key::from_secret(&secret) {
            break key;
        }
    };
    let cipher = transform(secret.as_mut(), true)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .share_mode(0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|_| "cannot create new Windows key file")?;
    inspect(&file)?;
    file.write_all(MAGIC)
        .and_then(|_| file.write_all(&cipher))
        .and_then(|_| file.sync_all())
        .map_err(|_| "key write failed; inspect incomplete file before retrying")?;
    Ok(key)
}
