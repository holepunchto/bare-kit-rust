#![allow(non_camel_case_types, non_upper_case_globals)]

use std::ffi::{c_char, c_int, c_void};

#[repr(C)]
pub struct bare_worklet_t {
    _private: [u8; 0],
}

#[repr(C)]
pub struct bare_ipc_t {
    _private: [u8; 0],
}

#[repr(C)]
pub struct bare_ipc_poll_t {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct bare_worklet_android_options_t {
    pub jvm: *mut c_void,
    pub context: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union bare_worklet_platform_options_t {
    pub android: bare_worklet_android_options_t,
    pub reserved: [*mut c_void; 8],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct bare_worklet_options_t {
    pub version: c_int,
    pub memory_limit: usize,
    pub platform: bare_worklet_platform_options_t,
}

pub type bare_worklet_suspend_cb = unsafe extern "C" fn(linger: c_int, data: *mut c_void);
pub type bare_worklet_wakeup_cb = unsafe extern "C" fn(deadline: c_int, data: *mut c_void);
pub type bare_worklet_idle_cb = unsafe extern "C" fn(data: *mut c_void);
pub type bare_worklet_resume_cb = unsafe extern "C" fn(data: *mut c_void);
pub type bare_worklet_exit_cb = unsafe extern "C" fn(exit_code: c_int, data: *mut c_void);
pub type bare_ipc_poll_cb = unsafe extern "C" fn(poll: *mut bare_ipc_poll_t, events: c_int);

pub const BARE_IPC_READ_BUFFER_SIZE: usize = 64 * 1024;

pub const bare_ipc_readable: c_int = 0x1;
pub const bare_ipc_writable: c_int = 0x2;

pub const bare_ipc_would_block: c_int = -1;
pub const bare_ipc_error: c_int = -2;

unsafe extern "C" {
    pub fn bare_worklet_optimize_for_memory(enabled: bool) -> c_int;

    pub fn bare_worklet_teardown() -> c_int;

    pub fn bare_worklet_create(
        options: *const bare_worklet_options_t,
        result: *mut *mut bare_worklet_t,
    ) -> c_int;

    pub fn bare_worklet_destroy(worklet: *mut bare_worklet_t);

    pub fn bare_worklet_on_suspend(
        worklet: *mut bare_worklet_t,
        cb: bare_worklet_suspend_cb,
        data: *mut c_void,
    ) -> c_int;

    pub fn bare_worklet_on_wakeup(
        worklet: *mut bare_worklet_t,
        cb: bare_worklet_wakeup_cb,
        data: *mut c_void,
    ) -> c_int;

    pub fn bare_worklet_on_idle(
        worklet: *mut bare_worklet_t,
        cb: bare_worklet_idle_cb,
        data: *mut c_void,
    ) -> c_int;

    pub fn bare_worklet_on_resume(
        worklet: *mut bare_worklet_t,
        cb: bare_worklet_resume_cb,
        data: *mut c_void,
    ) -> c_int;

    pub fn bare_worklet_on_exit(
        worklet: *mut bare_worklet_t,
        cb: bare_worklet_exit_cb,
        data: *mut c_void,
    ) -> c_int;

    pub fn bare_worklet_get_data(worklet: *mut bare_worklet_t) -> *mut c_void;

    pub fn bare_worklet_set_data(worklet: *mut bare_worklet_t, data: *mut c_void);

    pub fn bare_worklet_start(
        worklet: *mut bare_worklet_t,
        filename: *const c_char,
        source: *const c_void,
        len: usize,
        argc: c_int,
        argv: *const *const c_char,
    ) -> c_int;

    pub fn bare_worklet_suspend(worklet: *mut bare_worklet_t, linger: c_int) -> c_int;

    pub fn bare_worklet_resume(worklet: *mut bare_worklet_t) -> c_int;

    pub fn bare_worklet_wakeup(worklet: *mut bare_worklet_t, deadline: c_int) -> c_int;

    pub fn bare_worklet_terminate(worklet: *mut bare_worklet_t) -> c_int;

    pub fn bare_ipc_create(worklet: *mut bare_worklet_t, result: *mut *mut bare_ipc_t) -> c_int;

    pub fn bare_ipc_close(ipc: *mut bare_ipc_t);

    pub fn bare_ipc_end(ipc: *mut bare_ipc_t);

    pub fn bare_ipc_destroy(ipc: *mut bare_ipc_t);

    pub fn bare_ipc_read(ipc: *mut bare_ipc_t, data: *mut c_void, len: usize) -> c_int;

    pub fn bare_ipc_write(ipc: *mut bare_ipc_t, data: *const c_void, len: usize) -> c_int;

    pub fn bare_ipc_poll_create(ipc: *mut bare_ipc_t, result: *mut *mut bare_ipc_poll_t) -> c_int;

    pub fn bare_ipc_poll_destroy(poll: *mut bare_ipc_poll_t);

    pub fn bare_ipc_poll_get_data(poll: *mut bare_ipc_poll_t) -> *mut c_void;

    pub fn bare_ipc_poll_set_data(poll: *mut bare_ipc_poll_t, data: *mut c_void);

    pub fn bare_ipc_poll_get_ipc(poll: *mut bare_ipc_poll_t) -> *mut bare_ipc_t;

    pub fn bare_ipc_poll_start(
        poll: *mut bare_ipc_poll_t,
        events: c_int,
        cb: bare_ipc_poll_cb,
    ) -> c_int;

    pub fn bare_ipc_poll_stop(poll: *mut bare_ipc_poll_t) -> c_int;
}
