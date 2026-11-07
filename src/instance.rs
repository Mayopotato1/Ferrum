//! Session-scoped identity, independent of executable path/name. No IPC or activation.
use anyhow::Result;
use std::marker::PhantomData;
use windows::{
    Win32::{Foundation::*, System::Threading::*},
    core::PCWSTR,
};

pub const IDENTITY: &str = "Local\\SpotlightLauncherPrototype.35C83E18-76F3-4F51-A9C6-2E8C071833B5";
pub struct Instance {
    handle: HANDLE,
    // An owning mutex must be released on its acquiring thread.
    thread_bound: PhantomData<std::rc::Rc<()>>,
}
impl Instance {
    pub fn acquire() -> Result<Option<Self>> {
        Self::named(IDENTITY)
    }
    /// Public for isolated headless tests; never use the production identity in tests.
    pub fn named(name: &str) -> Result<Option<Self>> {
        let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        unsafe {
            let handle = CreateMutexW(None, false, PCWSTR(name.as_ptr()))?;
            // Wait, rather than ERROR_ALREADY_EXISTS, also recovers an abandoned owner
            // even if another process temporarily retains a handle to the object.
            let wait = WaitForSingleObject(handle, 0);
            if wait == WAIT_OBJECT_0 || wait == WAIT_ABANDONED {
                Ok(Some(Self {
                    handle,
                    thread_bound: PhantomData,
                }))
            } else {
                let error = windows::core::Error::from_thread();
                let _ = CloseHandle(handle);
                if wait == WAIT_TIMEOUT {
                    Ok(None)
                } else {
                    Err(error.into())
                }
            }
        }
    }
}
impl Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            let _ = ReleaseMutex(self.handle);
            let _ = CloseHandle(self.handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn abandoned_owner_is_recoverable_without_waiting_or_killing_a_process() {
        let name = format!("Local\\Spotlight-Abandon-Test-{}", std::process::id());
        let other = name.clone();
        let handle = std::thread::spawn(move || {
            let guard = Instance::named(&other).unwrap().unwrap();
            let handle = guard.handle.0 as usize;
            std::mem::forget(guard); // Thread termination abandons ownership.
            handle
        })
        .join()
        .unwrap();
        let recovered = Instance::named(&name).unwrap().unwrap();
        drop(recovered);
        unsafe {
            CloseHandle(HANDLE(handle as *mut _)).unwrap();
        }
        assert!(Instance::named(&name).unwrap().is_some());
    }
}
