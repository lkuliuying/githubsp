use std::{path::PathBuf, ptr::null_mut, sync::OnceLock};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, HWND, LPARAM, LRESULT, WPARAM,
    },
    System::{
        Com::CoTaskMemFree,
        DataExchange::COPYDATASTRUCT,
        LibraryLoader::GetModuleHandleW,
        Threading::{CreateMutexW, ReleaseMutex},
    },
    UI::{
        Shell::{FOLDERID_LocalAppData, SHGetKnownFolderPath},
        WindowsAndMessaging::*,
    },
};

static ACTIVATE: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

pub struct Startup {
    pub directory: PathBuf,
    pub locations: Option<githubsp_lib::relocation::Locations>,
    pub state: githubsp_lib::relocation::State,
}

pub fn startup() -> Result<Startup, String> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let explicit = if arguments.first().is_some_and(|a| a == "--restart-after") {
        if arguments.len() != 2 {
            return Err("重启参数无效".into());
        }
        let pid = arguments[1]
            .to_str()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|p| *p != 0)
            .ok_or("重启进程标识无效")?;
        wait_for_exit(pid)?;
        None
    } else {
        parse_arguments(arguments.into_iter())?
    };
    resolve_directory(explicit, || {
        let default_directory = data_directory_default()?;
        let config_file = default_directory
            .parent()
            .ok_or("应用目录无父目录")?
            .join("com.githubsp.desktop-location.json");
        Ok(githubsp_lib::relocation::Locations {
            default_directory,
            config_file,
        })
    })
}

pub(crate) fn resolve_directory(
    explicit: Option<PathBuf>,
    locations: impl FnOnce() -> Result<githubsp_lib::relocation::Locations, String>,
) -> Result<Startup, String> {
    if let Some(directory) = explicit {
        return Ok(Startup {
            directory,
            locations: None,
            state: Default::default(),
        });
    }
    let locations = locations()?;
    let state = locations.load().map_err(|e| e.to_string())?;
    Ok(Startup {
        directory: locations.selected(&state),
        locations: Some(locations),
        state,
    })
}

pub(crate) fn wait_for_exit(pid: u32) -> Result<(), String> {
    use windows_sys::Win32::{
        Foundation::{ERROR_INVALID_PARAMETER, WAIT_OBJECT_0},
        System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE},
    };
    if pid == std::process::id() {
        return Err("不能等待当前进程退出".into());
    }
    let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    if handle.is_null() {
        if unsafe { GetLastError() } == ERROR_INVALID_PARAMETER {
            return Ok(());
        }
        return Err(format!(
            "无法等待旧进程退出：{}",
            std::io::Error::last_os_error()
        ));
    }
    let result = unsafe { WaitForSingleObject(handle, 30_000) };
    unsafe {
        CloseHandle(handle);
    }
    if result != WAIT_OBJECT_0 {
        return Err("旧进程尚未退出，迁移数据已保留，请稍后重新打开 GitHubSP".into());
    }
    Ok(())
}

pub fn restart() -> Result<(), String> {
    restart_executable(&std::env::current_exe().map_err(|e| e.to_string())?)
}

fn restart_executable(executable: &std::path::Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    std::process::Command::new(executable)
        .args(["--restart-after", &std::process::id().to_string()])
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|e| format!("自动重启失败，请重新打开程序：{e}"))?;
    Ok(())
}

/// 仅依赖命名对象存续期，不跨线程转移 Windows 互斥锁的线程所有权。
pub struct DirectoryLock(HANDLE);
unsafe impl Send for DirectoryLock {}

impl DirectoryLock {
    pub fn acquire(directory: &std::path::Path) -> Result<Self, String> {
        Self::acquire_id(&instance_id(directory)?)
    }

    fn acquire_id(id: &str) -> Result<Self, String> {
        let name = wide(&format!("{id}-sim"));
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            unsafe {
                CloseHandle(handle);
            }
            return Err("该数据目录正在被另一个 GitHubSP 实例使用，请退出该实例后重试".into());
        }
        Ok(Self(handle))
    }
}

impl Drop for DirectoryLock {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn parse_arguments(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<Option<PathBuf>, String> {
    match arguments.next() {
        Some(flag) if flag == "--data-dir" => {
            let path = PathBuf::from(arguments.next().ok_or("--data-dir 缺少路径")?);
            if !path.is_absolute() || arguments.next().is_some() {
                return Err("--data-dir 需要一个绝对路径".into());
            }
            Ok(Some(path))
        }
        Some(_) => Err("不支持的启动参数；可使用 --data-dir 指定隔离数据目录".into()),
        None => Ok(None),
    }
}

pub fn instance_id(directory: &std::path::Path) -> Result<String, String> {
    let normal = data_directory_default()?;
    let directory = std::fs::canonicalize(directory).map_err(|e| e.to_string())?;
    if std::fs::canonicalize(normal).is_ok_and(|path| path == directory) {
        Ok("com.githubsp.desktop".into())
    } else {
        use sha2::{Digest, Sha256};
        Ok(format!(
            "com.githubsp.native.{:x}",
            Sha256::digest(directory.to_string_lossy().to_lowercase().as_bytes())
        ))
    }
}

fn data_directory_default() -> Result<PathBuf, String> {
    let mut buffer = null_mut();
    // 系统目录通过 Known Folder API 取得，不依赖可能被重写的环境变量。
    let result =
        unsafe { SHGetKnownFolderPath(&FOLDERID_LocalAppData, 0, null_mut(), &mut buffer) };
    if result < 0 || buffer.is_null() {
        return Err("无法读取应用目录".into());
    }
    let path = unsafe {
        let mut length = 0;
        while *buffer.add(length) != 0 {
            length += 1;
        }
        let path = PathBuf::from(String::from_utf16_lossy(std::slice::from_raw_parts(
            buffer, length,
        )));
        CoTaskMemFree(buffer.cast());
        path
    };
    Ok(path.join("com.githubsp.desktop"))
}

pub struct Instance {
    mutex: HANDLE,
    window: HWND,
    class: Vec<u16>,
}

impl Instance {
    pub fn acquire(
        id: &str,
        activate: impl Fn() + Send + Sync + 'static,
    ) -> Result<Option<Self>, String> {
        let name = wide(&format!("{id}-sim"));
        let class = wide(&format!("{id}-sic"));
        let title = wide(&format!("{id}-siw"));
        let mutex = unsafe { CreateMutexW(std::ptr::null(), 1, name.as_ptr()) };
        if mutex.is_null() {
            return Err(format!(
                "无法建立单实例锁：{}",
                std::io::Error::last_os_error()
            ));
        }
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            let existing = unsafe { FindWindowW(class.as_ptr(), title.as_ptr()) };
            if !existing.is_null() {
                let mut process = 0;
                unsafe {
                    GetWindowThreadProcessId(existing, &mut process);
                    AllowSetForegroundWindow(process);
                }
                // 与旧 Tauri 单实例协议兼容；只请求显示窗口，不传递文件或执行指令。
                let data = b"|githubsp-native\0";
                let payload = COPYDATASTRUCT {
                    dwData: 1542,
                    cbData: data.len() as u32,
                    lpData: data.as_ptr() as _,
                };
                let mut result = 0;
                unsafe {
                    SendMessageTimeoutW(
                        existing,
                        WM_COPYDATA,
                        0,
                        &payload as *const _ as _,
                        SMTO_ABORTIFHUNG,
                        2000,
                        &mut result,
                    );
                }
            }
            unsafe {
                CloseHandle(mutex);
            }
            if existing.is_null() {
                return Err("另一个 GitHubSP 正在启动或退出，请稍后重试".into());
            }
            return Ok(None);
        }
        let mut guard = Self {
            mutex,
            window: null_mut(),
            class,
        };
        ACTIVATE
            .set(Box::new(activate))
            .map_err(|_| "单实例激活处理器已注册")?;
        let module = unsafe { GetModuleHandleW(std::ptr::null()) };
        let definition = WNDCLASSW {
            lpfnWndProc: Some(instance_proc),
            hInstance: module,
            lpszClassName: guard.class.as_ptr(),
            ..Default::default()
        };
        if unsafe { RegisterClassW(&definition) } == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        guard.window = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                guard.class.as_ptr(),
                title.as_ptr(),
                WS_POPUP,
                0,
                0,
                0,
                0,
                null_mut(),
                null_mut(),
                module,
                std::ptr::null(),
            )
        };
        if guard.window.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(Some(guard))
    }
}

unsafe extern "system" fn instance_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_COPYDATA {
        if lparam != 0 && unsafe { (*(lparam as *const COPYDATASTRUCT)).dwData } == 1542 {
            if let Some(activate) = ACTIVATE.get() {
                activate();
            }
        }
        return 1;
    }
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

impl Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            if !self.window.is_null() {
                DestroyWindow(self.window);
            }
            UnregisterClassW(self.class.as_ptr(), GetModuleHandleW(std::ptr::null()));
            ReleaseMutex(self.mutex);
            CloseHandle(self.mutex);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_resolution_prefers_explicit_then_saved_then_default_without_fallback_on_bad_config()
    {
        let root = tempfile::tempdir().unwrap();
        let locations = githubsp_lib::relocation::Locations {
            default_directory: root.path().join("默认"),
            config_file: root.path().join("位置.json"),
        };
        let configured = || Ok(locations.clone());
        assert_eq!(
            resolve_directory(None, configured).unwrap().directory,
            locations.default_directory
        );
        let mut state = locations.load().unwrap();
        state.active = Some(root.path().join("自定义"));
        locations.save(&state).unwrap();
        assert_eq!(
            resolve_directory(None, configured).unwrap().directory,
            state.active.unwrap()
        );
        std::fs::write(&locations.config_file, b"broken").unwrap();
        assert!(resolve_directory(None, configured).is_err());
        let explicit = root.path().join("隔离");
        let result = resolve_directory(Some(explicit.clone()), || {
            panic!("显式参数不得读取正式配置")
        })
        .unwrap();
        assert_eq!(result.directory, explicit);
        assert!(result.locations.is_none());
        assert!(!explicit.exists());
    }

    #[test]
    fn migration_reservation_blocks_other_instances_and_missing_restart_program_is_reported() {
        let root = tempfile::tempdir().unwrap();
        let guard = DirectoryLock::acquire(root.path()).unwrap();
        assert!(DirectoryLock::acquire(root.path()).is_err());
        assert!(Instance::acquire(&instance_id(root.path()).unwrap(), || {}).is_err());
        drop(guard);
        let _again = DirectoryLock::acquire(root.path()).unwrap();
        assert!(restart_executable(&root.path().join("missing.exe")).is_err());
        assert!(wait_for_exit(std::process::id()).is_err());
    }

    #[test]
    fn isolated_directory_requires_an_absolute_path_and_no_extra_arguments() {
        let parse = |args: &[&str]| parse_arguments(args.iter().map(std::ffi::OsString::from));
        assert_eq!(parse(&[]).unwrap(), None);
        assert_eq!(
            parse(&["--data-dir", "C:\\中文测试"]).unwrap(),
            Some(PathBuf::from("C:\\中文测试"))
        );
        for args in [
            vec!["--data-dir"],
            vec!["--data-dir", "relative"],
            vec!["--data-dir", "C:\\test", "extra"],
            vec!["--unknown"],
        ] {
            assert!(parse(&args).is_err());
        }
    }

    #[test]
    fn isolated_instance_identity_uses_the_canonical_directory() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        assert_eq!(
            instance_id(first.path()).unwrap(),
            instance_id(&first.path().join(".")).unwrap()
        );
        assert_ne!(
            instance_id(first.path()).unwrap(),
            instance_id(second.path()).unwrap()
        );
        assert_ne!(instance_id(first.path()).unwrap(), "com.githubsp.desktop");
    }

    #[test]
    fn second_instance_activates_first_without_acquiring_its_data_lock() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let id = format!("com.githubsp.native.test.{}", std::process::id());
        let first = Instance::acquire(&id, move || {
            observed.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap()
        .unwrap();
        assert!(Instance::acquire(&id, || {}).unwrap().is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        drop(first);
        // 重复注册失败也必须释放新建互斥锁，不能阻止后续进程启动。
        assert!(Instance::acquire(&id, || {}).is_err());
        let name = wide(&format!("{id}-sim"));
        let mutex = unsafe { CreateMutexW(std::ptr::null(), 1, name.as_ptr()) };
        assert!(!mutex.is_null());
        assert_ne!(unsafe { GetLastError() }, ERROR_ALREADY_EXISTS);
        unsafe {
            ReleaseMutex(mutex);
            CloseHandle(mutex);
        }
    }
}
