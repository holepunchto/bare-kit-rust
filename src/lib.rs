//! Rust bindings for Bare Kit.

use std::ffi::{CString, OsStr, OsString, c_int, c_void};
use std::fmt;
use std::io::{self, Read, Write};
use std::path::Path;
use std::ptr;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

#[doc(hidden)]
pub mod sys;

/// A call into Bare Kit failed with the given status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error(pub i32);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Bare Kit failed with status {}", self.0)
    }
}

impl std::error::Error for Error {}

impl From<Error> for io::Error {
    fn from(err: Error) -> Self {
        io::Error::other(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

fn check(status: c_int) -> Result<()> {
    if status == 0 {
        Ok(())
    } else {
        Err(Error(status))
    }
}

fn c_string(value: &OsStr) -> CString {
    CString::new(value.as_encoded_bytes()).expect("String contains a NUL byte")
}

fn millis(duration: Option<Duration>) -> c_int {
    duration.map_or(-1, |duration| {
        duration.as_millis().min(c_int::MAX as u128) as c_int
    })
}

/// Trade performance for memory. Must be configured before starting the first
/// worklet.
pub fn optimize_for_memory(enabled: bool) -> Result<()> {
    check(unsafe { sys::bare_worklet_optimize_for_memory(enabled) })
}

/// Tear down the state shared by the worklets of the process. A process that
/// runs worklets for as long as it lives need not call it.
///
/// # Safety
///
/// Every worklet must have been dropped first, and this must be called at most
/// once, after which starting a worklet fails.
pub unsafe fn teardown() -> Result<()> {
    check(unsafe { sys::bare_worklet_teardown() })
}

/// Options for starting a worklet.
#[derive(Debug, Default, Clone)]
pub struct Builder {
    memory_limit: usize,
    source: Option<Vec<u8>>,
    args: Vec<OsString>,
    android: Option<sys::bare_worklet_android_options_t>,
}

impl Builder {
    /// The memory limit of each JavaScript heap in bytes. By default, the limit
    /// is inferred from the amount of physical memory of the device.
    pub fn memory_limit(mut self, bytes: usize) -> Self {
        self.memory_limit = bytes;
        self
    }

    /// The source of the module, which is otherwise read from its filename.
    pub fn source(mut self, source: impl Into<Vec<u8>>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Append an argument to `Bare.argv`.
    pub fn arg(mut self, arg: impl AsRef<OsStr>) -> Self {
        self.args.push(arg.as_ref().to_owned());
        self
    }

    /// Append arguments to `Bare.argv`.
    pub fn args<I>(self, args: I) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<OsStr>,
    {
        args.into_iter().fold(self, Self::arg)
    }

    /// The Java virtual machine, as a `JavaVM *`, and the application context,
    /// as a JNI reference to an `android.content.Context`, which apply on
    /// Android. By default, the virtual machine that loaded Bare Kit through
    /// `JNI_OnLoad()` is used, without an application context.
    ///
    /// # Safety
    ///
    /// `jvm` must be a valid `JavaVM *`, or null for the default, and `context`
    /// a JNI reference that is valid when the worklet starts, on a thread
    /// attached to the virtual machine.
    pub unsafe fn android(mut self, jvm: *mut c_void, context: *mut c_void) -> Self {
        self.android = Some(sys::bare_worklet_android_options_t { jvm, context });
        self
    }

    /// Start `filename` in a worklet with an IPC connection to it. A relative
    /// `filename` names a resource of the application.
    ///
    /// # Panics
    ///
    /// Panics if the filename or an argument contains a NUL byte.
    pub fn start(self, filename: impl AsRef<Path>) -> Result<Worklet> {
        let filename = c_string(filename.as_ref().as_os_str());

        let args: Vec<_> = self.args.iter().map(|arg| c_string(arg)).collect();

        let argv: Vec<_> = args
            .iter()
            .map(|arg| arg.as_ptr())
            .chain([ptr::null()])
            .collect();

        let worklet = Worklet::new(self.memory_limit, self.android)?;

        let (source, len) = match &self.source {
            Some(source) => (source.as_ptr().cast(), source.len()),
            None => (ptr::null(), 0),
        };

        check(unsafe {
            sys::bare_worklet_start(
                worklet.handle,
                filename.as_ptr(),
                source,
                len,
                args.len() as c_int,
                argv.as_ptr(),
            )
        })?;

        Ok(worklet)
    }
}

/// Start `filename` in a worklet with an IPC connection to it. Use
/// [`Worklet::builder()`] for more options.
pub fn start(filename: impl AsRef<Path>) -> Result<Worklet> {
    Worklet::builder().start(filename)
}

#[derive(Default)]
struct State {
    ready: c_int,
    wanted: c_int,
    polling: bool,
    exit_code: Option<i32>,
}

struct Shared {
    state: Mutex<State>,
    changed: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|err| err.into_inner())
    }
}

/// A Bare runtime running on a thread of its own, connected to its `Bare.IPC`.
///
/// Reading yields the data that the worklet writes to `Bare.IPC`, and writing
/// sends data to it. Both block until the IPC is ready, and `&Worklet` reads and
/// writes too, so one thread may read while another writes. Dropping the
/// worklet closes the IPC and terminates it without waiting for it to exit.
pub struct Worklet {
    handle: *mut sys::bare_worklet_t,
    ipc: *mut sys::bare_ipc_t,
    poll: *mut sys::bare_ipc_poll_t,
    shared: Arc<Shared>,
}

// Bare Kit may be called from any thread, and the state shared with its
// callbacks is behind a mutex.
unsafe impl Send for Worklet {}
unsafe impl Sync for Worklet {}

unsafe extern "C" fn on_exit(code: c_int, data: *mut c_void) {
    let shared = unsafe { Arc::from_raw(data as *const Shared) };

    shared.lock().exit_code = Some(code);
    shared.changed.notify_all();
}

unsafe extern "C" fn on_poll(poll: *mut sys::bare_ipc_poll_t, events: c_int) {
    let shared = unsafe { &*(sys::bare_ipc_poll_get_data(poll) as *const Shared) };

    let mut state = shared.lock();

    // Polls are level-triggered, so the poll is stopped until a waiter has
    // handled the events and starts it again.
    unsafe { sys::bare_ipc_poll_stop(poll) };

    state.ready |= events;
    state.wanted &= !events;
    state.polling = false;

    drop(state);

    shared.changed.notify_all();
}

impl Worklet {
    /// Options for starting a worklet.
    pub fn builder() -> Builder {
        Builder::default()
    }

    fn new(
        memory_limit: usize,
        android: Option<sys::bare_worklet_android_options_t>,
    ) -> Result<Self> {
        let mut platform = sys::bare_worklet_platform_options_t {
            reserved: [ptr::null_mut(); 8],
        };

        if let Some(android) = android {
            platform.android = android;
        }

        let options = sys::bare_worklet_options_t {
            version: 0,
            memory_limit,
            platform,
        };

        let mut handle = ptr::null_mut();

        check(unsafe { sys::bare_worklet_create(&options, &mut handle) })?;

        let mut worklet = Worklet {
            handle,
            ipc: ptr::null_mut(),
            poll: ptr::null_mut(),
            shared: Arc::new(Shared {
                state: Mutex::new(State::default()),
                changed: Condvar::new(),
            }),
        };

        check(unsafe { sys::bare_ipc_create(worklet.handle, &mut worklet.ipc) })?;
        check(unsafe { sys::bare_ipc_poll_create(worklet.ipc, &mut worklet.poll) })?;

        unsafe {
            sys::bare_ipc_poll_set_data(worklet.poll, Arc::as_ptr(&worklet.shared) as *mut c_void);

            // The exit callback is called exactly once, so it owns a reference.
            let data = Arc::into_raw(worklet.shared.clone()) as *mut c_void;

            if let Err(err) = check(sys::bare_worklet_on_exit(worklet.handle, on_exit, data)) {
                drop(Arc::from_raw(data as *const Shared));

                return Err(err);
            }
        }

        Ok(worklet)
    }

    // Retry `op` until it no longer would block, waiting for `event` between
    // attempts. Writing fails once the worklet has exited, while reading
    // continues until end-of-stream.
    fn wait_for(&self, event: c_int, mut op: impl FnMut() -> c_int) -> c_int {
        let mut state = self.shared.lock();

        loop {
            state.ready &= !event;

            let result = op();

            if result != sys::bare_ipc_would_block {
                return result;
            }

            state.wanted |= event;

            while state.ready & event == 0 {
                if event == sys::bare_ipc_writable && state.exit_code.is_some() {
                    state.wanted &= !event;

                    return sys::bare_ipc_error;
                }

                if !state.polling {
                    state.polling = true;

                    unsafe { sys::bare_ipc_poll_start(self.poll, state.wanted, on_poll) };
                }

                state = self
                    .shared
                    .changed
                    .wait(state)
                    .unwrap_or_else(|err| err.into_inner());
            }
        }
    }

    /// End the data written to the worklet, after which `Bare.IPC` reads
    /// end-of-stream once drained while reading continues.
    pub fn end(&self) {
        unsafe { sys::bare_ipc_end(self.ipc) };
    }

    /// Suspend the worklet, emitting `suspend` on `Bare`. It may keep running
    /// for `linger` before it goes idle, or for as long as the platform allows
    /// if `None`.
    pub fn suspend(&self, linger: Option<Duration>) {
        unsafe { sys::bare_worklet_suspend(self.handle, millis(linger)) };
    }

    /// Resume a suspended worklet, emitting `resume` on `Bare`.
    pub fn resume(&self) {
        unsafe { sys::bare_worklet_resume(self.handle) };
    }

    /// Wake up a suspended worklet, emitting `wakeup` on `Bare`. It may run for
    /// `deadline` before it goes idle again, or for as long as the platform
    /// allows if `None`.
    pub fn wakeup(&self, deadline: Option<Duration>) {
        unsafe { sys::bare_worklet_wakeup(self.handle, millis(deadline)) };
    }

    /// Ask the worklet to exit unless it has already exited.
    pub fn terminate(&self) {
        unsafe { sys::bare_worklet_terminate(self.handle) };
    }

    /// The exit code of the worklet, or `None` while it is running.
    pub fn exit_code(&self) -> Option<i32> {
        self.shared.lock().exit_code
    }

    /// Wait for the worklet to exit and return its exit code.
    pub fn wait(&self) -> i32 {
        let state = self.shared.lock();

        let state = self
            .shared
            .changed
            .wait_while(state, |state| state.exit_code.is_none())
            .unwrap_or_else(|err| err.into_inner());

        state.exit_code.unwrap()
    }
}

impl Read for &Worklet {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let len = buf.len().min(c_int::MAX as usize);

        let read = self.wait_for(sys::bare_ipc_readable, || unsafe {
            sys::bare_ipc_read(self.ipc, buf.as_mut_ptr().cast(), len)
        });

        Ok(read.max(0) as usize)
    }
}

impl Read for Worklet {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&*self).read(buf)
    }
}

impl Write for &Worklet {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let len = buf.len().min(c_int::MAX as usize);

        let written = self.wait_for(sys::bare_ipc_writable, || unsafe {
            sys::bare_ipc_write(self.ipc, buf.as_ptr().cast(), len)
        });

        if written < 0 {
            return Err(io::ErrorKind::BrokenPipe.into());
        }

        Ok(written as usize)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Write for Worklet {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&*self).write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for Worklet {
    fn drop(&mut self) {
        unsafe {
            if !self.ipc.is_null() {
                sys::bare_ipc_close(self.ipc);
            }

            sys::bare_worklet_terminate(self.handle);

            if !self.poll.is_null() {
                sys::bare_ipc_poll_destroy(self.poll);
            }

            if !self.ipc.is_null() {
                sys::bare_ipc_destroy(self.ipc);
            }

            sys::bare_worklet_destroy(self.handle);
        }
    }
}
