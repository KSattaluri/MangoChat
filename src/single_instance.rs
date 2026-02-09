#[cfg(windows)]
mod imp {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
    use windows::Win32::System::Threading::CreateMutexW;

    pub struct SingleInstanceGuard {
        handle: HANDLE,
    }

    impl Drop for SingleInstanceGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }

    pub fn acquire(app_id: &str) -> Option<SingleInstanceGuard> {
        let mut name: Vec<u16> = format!("Local\\{}", app_id).encode_utf16().collect();
        name.push(0);

        let handle = unsafe { CreateMutexW(None, true, PCWSTR(name.as_ptr())) }.ok()?;
        let already_exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;

        if already_exists {
            unsafe {
                let _ = CloseHandle(handle);
            }
            None
        } else {
            Some(SingleInstanceGuard { handle })
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub struct SingleInstanceGuard;

    pub fn acquire(_app_id: &str) -> Option<SingleInstanceGuard> {
        Some(SingleInstanceGuard)
    }
}

pub use imp::acquire;

