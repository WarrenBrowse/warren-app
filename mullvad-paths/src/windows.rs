use crate::{Error, Result, UserPermissions};
use once_cell::sync::OnceCell;
use std::{
    ffi::c_void,
    io, mem,
    os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle},
    path::{Path, PathBuf},
    ptr,
};
use widestring::{U16CString, WideCStr, u16cstr};
use windows_sys::{
    Win32::{
        Foundation::{
            ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, GENERIC_ALL, GENERIC_EXECUTE, GENERIC_READ,
            GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE, LUID, LocalFree, S_OK,
        },
        Security::{
            self, AdjustTokenPrivileges,
            Authorization::{
                EXPLICIT_ACCESS_W, GetSecurityInfo, NO_MULTIPLE_TRUSTEE, SE_FILE_OBJECT,
                SET_ACCESS, SetEntriesInAclW, SetSecurityInfo, TRUSTEE_IS_GROUP, TRUSTEE_IS_SID,
                TRUSTEE_W,
            },
            CreateWellKnownSid, EqualSid, GetTokenInformation, ImpersonateSelf, IsWellKnownSid,
            LUID_AND_ATTRIBUTES, LookupPrivilegeValueW, NO_INHERITANCE, PSECURITY_DESCRIPTOR, PSID,
            RevertToSelf, SE_PRIVILEGE_ENABLED, SUB_CONTAINERS_AND_OBJECTS_INHERIT,
            SecurityImpersonation, TOKEN_ADJUST_PRIVILEGES, TOKEN_DUPLICATE, TOKEN_IMPERSONATE,
            TOKEN_PRIVILEGES, TOKEN_QUERY, TOKEN_USER, TokenUser, WinAuthenticatedUserSid,
            WinBuiltinAdministratorsSid, WinLocalSystemSid,
        },
        Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_ATTRIBUTE_DIRECTORY,
            FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
            FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, GetFileInformationByHandle,
            MAX_SID_SIZE, OPEN_EXISTING, READ_CONTROL, WRITE_DAC, WRITE_OWNER,
        },
        System::{
            Com::CoTaskMemFree,
            ProcessStatus::EnumProcesses,
            Threading::{
                GetCurrentThread, OpenProcess, OpenProcessToken, OpenThreadToken,
                PROCESS_QUERY_INFORMATION,
            },
        },
        UI::Shell::{
            FOLDERID_LocalAppData, FOLDERID_System, KF_FLAG_DEFAULT, SHGetKnownFolderPath,
        },
    },
    core::{GUID, PWSTR},
};

// Renamed to `Warren VPN` for the Warren fork - see
// `unix.rs::PRODUCT_NAME` for the rationale (anti-collision with
// Mullvad upstream). Non-prod product environments get their own
// suffixed directory (`Warren VPN Beta`, ...) so a beta install
// coexists with prod without sharing state.
pub const PRODUCT_NAME: &str = warren_product_env::DISPLAY_NAME;

pub fn get_allusersprofile_dir() -> Result<PathBuf> {
    match std::env::var_os("ALLUSERSPROFILE") {
        Some(dir) => Ok(PathBuf::from(&dir)),
        None => Err(Error::NoProgramDataDir),
    }
}

/// This recursively creates directories, if set_security_permissions is true it will set
/// file permissions corresponding to Authenticated Users - Read Only and Administrators - Full
/// Access. Only directories that do not already exist and the leaf directory will have their
/// permissions set.
#[cfg(windows)]
pub fn create_dir(path: PathBuf, user_permissions: Option<UserPermissions>) -> Result<PathBuf> {
    if let Some(user_permissions) = user_permissions {
        create_dir_recursive_with_permissions(&path, user_permissions)?;
    } else {
        std::fs::create_dir_all(&path).map_err(|e| {
            Error::CreateDirFailed(
                format!("Could not create directory at {}", path.display()),
                e,
            )
        })?;
    }
    Ok(path)
}

impl UserPermissions {
    fn flags(self) -> u32 {
        let mut flags = 0;
        if self.read {
            flags |= GENERIC_READ;
        }
        if self.write {
            flags |= GENERIC_WRITE;
        }
        if self.execute {
            flags |= GENERIC_EXECUTE;
        }
        flags
    }
}

/// If directory at path already exists, set permissions for it.
/// If directory at path don't exist but parent does, create directory and set permissions.
/// If parent directory at path does not exist then recurse and create parent directory and set
/// permissions for it, then create child directory and set permissions.
/// This does not set permissions for parent directories that already exists.
fn create_dir_recursive_with_permissions(
    path: &Path,
    user_permissions: UserPermissions,
) -> Result<()> {
    // No directory to create
    if path == Path::new("") {
        return Ok(());
    }
    reject_reparse_points(path)?;

    match std::fs::create_dir(path) {
        Ok(()) => {
            return set_security_permissions(path, user_permissions);
        }
        // Could not find parent directory, try creating parent
        Err(e) if e.kind() == io::ErrorKind::NotFound => (),
        // The directory already exists. Any account may have created it before the first install,
        // so it is reused only when an administrators group or SYSTEM owns it; another account's
        // directory is replaced, as the Unix version does, rather than handed the privileged ACL.
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists && path.is_dir() => {
            let directory = open_directory_for_security(path)?;
            if owner_is_privileged(&directory)? {
                return apply_security_permissions(&directory, user_permissions);
            }
            drop(directory);
            log::warn!("Replacing a directory that an unprivileged account created");
            std::fs::remove_dir_all(path)
                .map_err(|e| Error::RemoveDir(path.display().to_string(), e))?;
            std::fs::create_dir(path)
                .map_err(|e| Error::CreateDirFailed(path.display().to_string(), e))?;
            return set_security_permissions(path, user_permissions);
        }
        Err(e) => {
            return Err(Error::CreateDirFailed(
                format!("Could not create directory at {}", path.display()),
                e,
            ));
        }
    }

    match path.parent() {
        // Create parent directory
        Some(parent) => create_dir_recursive_with_permissions(parent, user_permissions)?,
        None => {
            // Reached the top of the tree but when creating directories only got NotFound for some
            // reason
            return Err(Error::CreateDirFailed(
                path.display().to_string(),
                io::Error::other("reached top of directory tree but could not create directory"),
            ));
        }
    }

    std::fs::create_dir(path).map_err(|e| Error::CreateDirFailed(path.display().to_string(), e))?;
    set_security_permissions(path, user_permissions)
}

/// Refuses `path` when it, or a directory above it, is a reparse point (a junction or a symbolic
/// link). The privileged directories live where any account can create entries before the first
/// install, and the security descriptor applied by name below lands on the target of a link, so a
/// junction planted there would hand its target to whoever chose it.
fn reject_reparse_points(path: &Path) -> Result<()> {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    for ancestor in path.ancestors() {
        let Ok(metadata) = std::fs::symlink_metadata(ancestor) else {
            continue;
        };
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(Error::ReparsePoint(ancestor.display().to_string()));
        }
    }
    Ok(())
}

/// Recursively creates directories for the given path with permissions that give full access to
/// admins and read only access to authenticated users. If any of the directories already exist this
/// will not return an error, instead it will apply the permissions and if successful return Ok(()).
pub fn create_privileged_directory(path: &Path) -> Result<()> {
    create_dir_recursive_with_permissions(path, UserPermissions::read_only())
}

/// Opens the directory at `path` itself, never what a reparse point there leads to, with the rights
/// to read and replace its security descriptor. Applying the descriptor through this handle rather
/// than by name leaves no window for a junction to be swapped in after the checks.
fn open_directory_for_security(path: &Path) -> Result<OwnedHandle> {
    let wide_path = U16CString::from_os_str(path)?;
    let failed = |error| Error::SetDirPermissionFailed(path.display().to_string(), error);
    // SAFETY: `wide_path` is NUL-terminated and outlives the call; the security attributes and the
    // template are null, which the API accepts.
    let raw = unsafe {
        CreateFileW(
            wide_path.as_ptr(),
            READ_CONTROL | WRITE_DAC | WRITE_OWNER,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            ptr::null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(failed(io::Error::last_os_error()));
    }
    // SAFETY: `CreateFileW` succeeded, so `raw` is an open handle that nothing else owns.
    let handle = unsafe { OwnedHandle::from_raw_handle(raw) };

    // SAFETY: an all-zero `BY_HANDLE_FILE_INFORMATION` is a valid value of this plain C struct.
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { mem::zeroed() };
    // SAFETY: the handle is open, and `info` is a live local the call fills in.
    if unsafe { GetFileInformationByHandle(handle.as_raw_handle(), &raw mut info) } == 0 {
        return Err(failed(io::Error::last_os_error()));
    }
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(Error::ReparsePoint(path.display().to_string()));
    }
    if info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return Err(failed(io::Error::other("not a directory")));
    }
    Ok(handle)
}

/// Whether the directory behind `handle` is owned by an administrators group or by SYSTEM.
fn owner_is_privileged(handle: &OwnedHandle) -> Result<bool> {
    let mut owner: PSID = ptr::null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
    // SAFETY: the handle is open with READ_CONTROL. Only the owner is asked for, so every other
    // out-pointer is null; `owner` points into `descriptor`, which is freed below after its last
    // use.
    let result = unsafe {
        GetSecurityInfo(
            handle.as_raw_handle(),
            SE_FILE_OBJECT,
            Security::OWNER_SECURITY_INFORMATION,
            &raw mut owner,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            &raw mut descriptor,
        )
    };
    if result != ERROR_SUCCESS {
        return Err(Error::GetDirPermissionFailed(
            String::from("GetSecurityInfo failed"),
            io::Error::from_raw_os_error(i32::try_from(result).unwrap_or(i32::MAX)),
        ));
    }
    // SAFETY: `owner` is the valid SID `GetSecurityInfo` returned inside `descriptor`.
    let privileged = unsafe {
        IsWellKnownSid(owner, WinBuiltinAdministratorsSid) != 0
            || IsWellKnownSid(owner, WinLocalSystemSid) != 0
    };
    // SAFETY: `descriptor` was allocated by `GetSecurityInfo` for the caller to free with
    // `LocalFree`, and `owner`, which points into it, is not used after this.
    unsafe { LocalFree(descriptor) };
    Ok(privileged)
}

/// Sets security permissions for path such that admin has full ownership and access while
/// authenticated users only have read access.
fn set_security_permissions(path: &Path, user_permissions: UserPermissions) -> Result<()> {
    let directory = open_directory_for_security(path)?;
    apply_security_permissions(&directory, user_permissions)
}

/// [`set_security_permissions`] on a directory already opened by
/// [`open_directory_for_security`].
fn apply_security_permissions(
    directory: &OwnedHandle,
    user_permissions: UserPermissions,
) -> Result<()> {
    let security_information = Security::DACL_SECURITY_INFORMATION
        | Security::PROTECTED_DACL_SECURITY_INFORMATION
        | Security::GROUP_SECURITY_INFORMATION
        | Security::OWNER_SECURITY_INFORMATION;

    let mut admin_psid = [0u8; MAX_SID_SIZE as usize];
    let mut admin_psid_len =
        u32::try_from(admin_psid.len()).expect("admin_psid length fits in u32");

    // SAFETY: The pointer to the PSID is valid for writes of `admin_psid_len` bytes
    if unsafe {
        CreateWellKnownSid(
            WinBuiltinAdministratorsSid,
            ptr::null_mut(),
            admin_psid.as_mut_ptr().cast::<c_void>(),
            &raw mut admin_psid_len,
        )
    } == 0
    {
        return Err(Error::SetDirPermissionFailed(
            String::from("Could not create admin SID"),
            io::Error::last_os_error(),
        ));
    }

    let trustee = TRUSTEE_W {
        pMultipleTrustee: ptr::null_mut(),
        MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
        TrusteeForm: TRUSTEE_IS_SID,
        TrusteeType: TRUSTEE_IS_GROUP,
        ptstrName: admin_psid.as_mut_ptr() as *mut _,
    };

    let admin_ea = EXPLICIT_ACCESS_W {
        grfAccessPermissions: GENERIC_ALL,
        grfAccessMode: SET_ACCESS,
        grfInheritance: NO_INHERITANCE | SUB_CONTAINERS_AND_OBJECTS_INHERIT,
        Trustee: trustee,
    };

    let mut au_psid = [0u8; MAX_SID_SIZE as usize];
    let mut au_psid_len = u32::try_from(au_psid.len()).expect("au_psid length fits in u32");

    // SAFETY: The pointer to the PSID is valid for writes of `au_psid_len` bytes
    if unsafe {
        CreateWellKnownSid(
            WinAuthenticatedUserSid,
            ptr::null_mut(),
            au_psid.as_mut_ptr().cast::<c_void>(),
            &raw mut au_psid_len,
        )
    } == 0
    {
        return Err(Error::SetDirPermissionFailed(
            String::from("Could not create authenticated users SID"),
            io::Error::last_os_error(),
        ));
    }

    let trustee = TRUSTEE_W {
        pMultipleTrustee: ptr::null_mut(),
        MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
        TrusteeForm: TRUSTEE_IS_SID,
        TrusteeType: TRUSTEE_IS_GROUP,
        ptstrName: au_psid.as_mut_ptr().cast(),
    };

    let authenticated_users_ea = EXPLICIT_ACCESS_W {
        grfAccessPermissions: user_permissions.flags(),
        grfAccessMode: SET_ACCESS,
        grfInheritance: NO_INHERITANCE | SUB_CONTAINERS_AND_OBJECTS_INHERIT,
        Trustee: trustee,
    };

    let ea_entries = [admin_ea, authenticated_users_ea];
    let mut new_dacl = ptr::null_mut();

    // SAFETY: `ea_entries` is valid for reads of `ea_entries.len()` elements
    // `new_dacl` is a valid pointer to an ACL pointer
    let result = unsafe {
        SetEntriesInAclW(
            u32::try_from(ea_entries.len()).expect("number of entries fits in u32"),
            ea_entries.as_ptr(),
            ptr::null(),
            &raw mut new_dacl,
        )
    };
    if result != ERROR_SUCCESS {
        return Err(Error::SetDirPermissionFailed(
            String::from("SetEntriesInAclW failed"),
            io::Error::from_raw_os_error(
                i32::try_from(result).expect("result does not fit in i32"),
            ),
        ));
    }
    // new_dacl is now allocated and must be freed with FreeLocal

    // SAFETY: the handle is open with WRITE_DAC and WRITE_OWNER, both SIDs live in local arrays
    // and `new_dacl` was built by `SetEntriesInAclW` above; all outlive the call.
    let result = unsafe {
        SetSecurityInfo(
            directory.as_raw_handle(),
            SE_FILE_OBJECT,
            security_information,
            admin_psid.as_mut_ptr().cast(),
            admin_psid.as_mut_ptr().cast(),
            new_dacl,
            ptr::null(),
        )
    };

    // SAFETY: `new_dacl` is a valid pointer since `SetEntriesInAclW` succeeded
    unsafe { LocalFree(new_dacl.cast()) };

    if result != ERROR_SUCCESS {
        Err(Error::SetDirPermissionFailed(
            String::from("SetSecurityInfo failed"),
            io::Error::from_raw_os_error(
                i32::try_from(result).expect("result does not fit in i32"),
            ),
        ))
    } else {
        Ok(())
    }
}

/// Get local AppData path for the system service user.
pub fn get_system_service_appdata() -> io::Result<PathBuf> {
    static APPDATA_PATH: OnceCell<PathBuf> = OnceCell::new();

    APPDATA_PATH
        .get_or_try_init(|| {
            let join_handle = std::thread::spawn(|| {
                impersonate_self(|| {
                    let user_token = get_system_user_token()?;
                    // SAFETY: `FOLDERID_LocalAppData` is a valid known folder ID
                    unsafe {
                        get_known_folder_path(
                            &FOLDERID_LocalAppData,
                            KF_FLAG_DEFAULT,
                            user_token.as_ref().map(|t| t.as_handle()),
                        )
                    }
                })
                .or_else(|error| {
                    log::error!("Failed to get AppData path: {error}");
                    infer_appdata_from_system_directory()
                })
            });
            join_handle.join().expect("ImpersonateSelf should succeed")
        })
        .cloned()
}

/// Get user token for the system service user. Requires elevated privileges to work.
/// Useful for deducing the config path for the daemon on Windows when running as a user that
/// isn't the system service.
/// If the current user is system, this function succeeds and returns a NULL handle
fn get_system_user_token() -> io::Result<Option<OwnedHandle>> {
    let thread_token = get_current_thread_token()?;

    if is_local_system_user_token(&thread_token)? {
        return Ok(None);
    }

    let system_debug_priv = u16cstr!("SeDebugPrivilege");
    adjust_token_privilege(&thread_token, system_debug_priv, true)?;

    let find_result = find_process(|process_handle| {
        let process_token = open_process_token(
            &process_handle,
            GENERIC_READ | TOKEN_IMPERSONATE | TOKEN_DUPLICATE,
        )
        .ok()?;

        match is_local_system_user_token(&process_token) {
            Ok(true) => Some(process_token),
            _ => None,
        }
    });

    if let Err(err) = adjust_token_privilege(&thread_token, system_debug_priv, false) {
        log::error!("Failed to drop SeDebugPrivilege: {}", err);
    }

    find_result.map(Some)
}

fn open_process_token(process: &impl AsRawHandle, access: u32) -> io::Result<OwnedHandle> {
    let mut process_token = ptr::null_mut();
    // SAFETY: `process` is a valid handle
    if unsafe { OpenProcessToken(process.as_raw_handle(), access, &raw mut process_token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `process_token` is a valid handle since `OpenProcessToken` succeeded
    Ok(unsafe { OwnedHandle::from_raw_handle(process_token) })
}

/// If all else fails, infer the AppData path from the system directory.
fn infer_appdata_from_system_directory() -> io::Result<PathBuf> {
    // SAFETY: `FOLDERID_System` is a valid known folder ID
    let mut sysdir = unsafe { get_known_folder_path(&FOLDERID_System, KF_FLAG_DEFAULT, None) }?;
    sysdir.extend(["config", "systemprofile", "AppData", "Local"]);
    Ok(sysdir)
}

fn get_current_thread_token() -> std::io::Result<OwnedHandle> {
    let mut token_handle: HANDLE = ptr::null_mut();
    // SAFETY: `GetCurrentThread` always returns a valid handle
    if unsafe {
        OpenThreadToken(
            GetCurrentThread(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            0,
            &raw mut token_handle,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: `token_handle` is a valid handle since `OpenThreadToken` succeeded
    Ok(unsafe { OwnedHandle::from_raw_handle(token_handle) })
}

/// Run provided closure in the security context of the calling process' impersonation token.
///
/// # Panics
///
/// If privileges can not be dropped after running `func`, the running process is shut down.
fn impersonate_self<T>(func: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
    // SAFETY: SecurityImpersonation is a valid ImpersonationLevel.
    if unsafe { ImpersonateSelf(SecurityImpersonation) } == 0 {
        return Err(std::io::Error::last_os_error());
    }

    let result = func();

    // SAFETY: Must be called after a successful call to ImpersonateSelf.
    if unsafe { RevertToSelf() } == 0 {
        // The Windows documentation *strongly* suggest that the process should shut down if
        // RevertToSelf fails. A failure to do so means that the current process keep running in
        // an unintended context.
        log::error!("RevertToSelf failed: {}", io::Error::last_os_error());
        panic!("RevertToSelf failed. Aborting");
    }

    result
}

fn adjust_token_privilege(
    token_handle: &impl AsRawHandle,
    privilege: &WideCStr,
    enable: bool,
) -> std::io::Result<()> {
    let mut privilege_luid = LUID::default();

    // SAFETY: `privilege` is a valid null-terminated string, and `privilege_luid` points to a LUID
    if unsafe { LookupPrivilegeValueW(ptr::null(), privilege.as_ptr(), &raw mut privilege_luid) }
        == 0
    {
        return Err(std::io::Error::last_os_error());
    }

    let privileges = TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [LUID_AND_ATTRIBUTES {
            Luid: privilege_luid,
            Attributes: if enable { SE_PRIVILEGE_ENABLED } else { 0 },
        }],
    };
    // SAFETY: All pointers are valid
    let result = unsafe {
        AdjustTokenPrivileges(
            token_handle.as_raw_handle(),
            0,
            &raw const privileges,
            0,
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    // Terrible interface.
    //  Odd 2018
    let last_error = std::io::Error::last_os_error();
    if result == 0 || last_error.raw_os_error() != Some(ERROR_SUCCESS as i32) {
        return Err(last_error);
    }

    Ok(())
}

/// Retrieve path to a known folder for a specific user token.
///
/// # Safety
///
/// `folder_id` must be a valid pointer to a known folder ID GUID.
unsafe fn get_known_folder_path(
    folder_id: *const GUID,
    flags: i32,
    user_token: Option<BorrowedHandle<'_>>,
) -> std::io::Result<PathBuf> {
    let mut folder_path: PWSTR = ptr::null_mut();
    // SAFETY: All arguments are valid
    let status = unsafe {
        SHGetKnownFolderPath(
            folder_id,
            flags as u32,
            user_token
                .map(|h| h.as_raw_handle())
                .unwrap_or(ptr::null_mut()),
            &raw mut folder_path,
        )
    };
    let result = if status == S_OK {
        // SAFETY: `folder_path` is valid and null-terminated since `SHGetKnownFolderPath` succeeded
        let path = unsafe { WideCStr::from_ptr_str(folder_path) };
        Ok(PathBuf::from(path.to_os_string()))
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Can't find known folder {:?}", &folder_id),
        ))
    };

    // SAFETY: `folder_path` was allocated by `SHGetKnownFolderPath` and must be freed with `CoTaskMemFree
    unsafe { CoTaskMemFree(folder_path as *mut _) };
    result
}

/// Enumerate over all processes until `handle_process` returns a result or until there are
/// no more processes left. In the latter case, an error is returned.
fn find_process<T>(handle_process: impl Fn(BorrowedHandle<'_>) -> Option<T>) -> io::Result<T> {
    let mut pid_buffer = vec![0u32; 2048];
    let mut num_procs: u32 =
        u32::try_from(pid_buffer.len()).expect("Number of processes IDs fit in u32");

    let bytes_available = num_procs * (mem::size_of::<u32>() as u32);
    let mut bytes_written = 0;

    // SAFETY: `pid_buffer` is valid for writes of `bytes_available` bytes
    if unsafe {
        EnumProcesses(
            pid_buffer.as_mut_ptr(),
            bytes_available,
            &raw mut bytes_written,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }

    num_procs = bytes_written / (mem::size_of::<u32>() as u32);
    pid_buffer.resize(num_procs as usize, 0);

    pid_buffer
        .into_iter()
        .find_map(|process| {
            // SAFETY: Trivially safe
            let process_handle = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION, 0, process) };
            if process_handle.is_null() {
                return None;
            }
            // SAFETY: `process_handle` is a valid handle since `OpenProcess` succeeded
            let process_handle = unsafe { OwnedHandle::from_raw_handle(process_handle) };
            handle_process(process_handle.as_handle())
        })
        .ok_or(io::Error::new(
            io::ErrorKind::NotFound,
            "Could not find matching process",
        ))
}

/// The `TOKEN_USER` of a token, in storage aligned for it.
struct TokenUserInfo(Vec<u64>);

impl TokenUserInfo {
    /// The account SID of the token, valid as long as `self`.
    fn sid(&self) -> PSID {
        // SAFETY: the buffer holds the `TOKEN_USER` that `GetTokenInformation` wrote for the
        // `TokenUser` class, and a `Vec<u64>` is aligned for it.
        unsafe { (*(self.0.as_ptr() as *const TOKEN_USER)).User.Sid }
    }
}

fn token_user(token: &impl AsRawHandle) -> io::Result<TokenUserInfo> {
    let mut token_info = vec![0u64; 1024 / mem::size_of::<u64>()];

    loop {
        let mut returned_info_len = 0;

        // SAFETY: `token` is a valid handle, and `token_info` is valid for writes of its size in
        // bytes.
        let info_result = unsafe {
            GetTokenInformation(
                token.as_raw_handle(),
                TokenUser,
                token_info.as_mut_ptr().cast::<c_void>(),
                u32::try_from(mem::size_of_val(&token_info[..])).expect("len must fit in u32"),
                &raw mut returned_info_len,
            )
        };

        let err = io::Error::last_os_error();
        if info_result == 0 && err.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER as i32) {
            log::error!("Failed to obtain token information: {}", err);
            return Err(err);
        }
        if info_result != 0 {
            return Ok(TokenUserInfo(token_info));
        }
        let needed = usize::try_from(returned_info_len).expect("u32 must fit in usize");
        token_info.resize(needed.div_ceil(mem::size_of::<u64>()), 0);
    }
}

fn is_local_system_user_token(token: &impl AsRawHandle) -> io::Result<bool> {
    let token_user = token_user(token)?;

    let mut local_system_sid = [0u8; MAX_SID_SIZE as usize];
    let mut local_system_size =
        u32::try_from(local_system_sid.len()).expect("local_system_sid length fits in u32");

    // SAFETY: `local_system_sid` is valid for writes of `local_system_size` bytes
    if unsafe {
        CreateWellKnownSid(
            WinLocalSystemSid,
            std::ptr::null_mut(),
            local_system_sid.as_mut_ptr().cast::<c_void>(),
            &raw mut local_system_size,
        )
    } == 0
    {
        let err = io::Error::last_os_error();
        log::error!("CreateWellKnownSid failed: {}", err);
        return Err(err);
    }

    // SAFETY: Both arguments point to valid security identifiers
    Ok(unsafe {
        EqualSid(
            token_user.sid(),
            local_system_sid.as_mut_ptr().cast::<c_void>(),
        )
    } != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    fn scratch(tag: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("warren-paths-{tag}-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_privileged_directory_is_never_made_through_a_junction() {
        let root = scratch("junction");
        let target = root.join("chosen-by-another-account");
        std::fs::create_dir(&target).unwrap();
        let junction = root.join("product");
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&target)
            .status()
            .unwrap();
        assert!(status.success(), "mklink /J failed");

        let result = create_privileged_directory(&junction.join("cache"));

        assert!(matches!(result, Err(Error::ReparsePoint(_))), "{result:?}");
        assert!(!target.join("cache").exists());
        let _ = std::fs::remove_dir(&junction);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Makes the running account, rather than an administrators group, the owner of `path`.
    fn give_to_current_account(path: &Path) {
        let process_token =
            // SAFETY: `GetCurrentProcess` returns a pseudo handle that needs no closing.
            open_process_token(&unsafe { BorrowedHandle::borrow_raw(GetCurrentProcess()) }, TOKEN_QUERY)
                .unwrap();
        let user = token_user(&process_token).unwrap();
        let directory = open_directory_for_security(path).unwrap();
        // SAFETY: the handle is open with WRITE_OWNER, and the owner SID lives in `user`, which
        // outlives the call. No other part of the descriptor is set.
        let result = unsafe {
            SetSecurityInfo(
                directory.as_raw_handle(),
                SE_FILE_OBJECT,
                Security::OWNER_SECURITY_INFORMATION,
                user.sid(),
                ptr::null_mut(),
                ptr::null(),
                ptr::null(),
            )
        };
        assert_eq!(result, ERROR_SUCCESS, "SetSecurityInfo failed");
    }

    #[test]
    fn a_directory_it_creates_is_owned_by_the_administrators() {
        let root = scratch("owner");
        let product = root.join("product");

        create_privileged_directory(&product).unwrap();

        let directory = open_directory_for_security(&product).unwrap();
        assert!(owner_is_privileged(&directory).unwrap());
        drop(directory);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_existing_directory_another_account_owns_is_replaced_not_reused() {
        let root = scratch("foreign-owner");
        let product = root.join("product");
        std::fs::create_dir(&product).unwrap();
        std::fs::write(product.join("planted"), b"chosen by another account").unwrap();
        give_to_current_account(&product);
        assert!(!owner_is_privileged(&open_directory_for_security(&product).unwrap()).unwrap());

        create_privileged_directory(&product).unwrap();

        assert!(
            !product.join("planted").exists(),
            "the foreign directory was reused"
        );
        let directory = open_directory_for_security(&product).unwrap();
        assert!(owner_is_privileged(&directory).unwrap());
        drop(directory);
        let _ = std::fs::remove_dir_all(&root);
    }
}
