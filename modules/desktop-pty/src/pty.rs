//! The pseudo-terminal, across the C ABI.
//!
//! A view does four things with a terminal: it reads the bytes the shell printed, writes the bytes a
//! user typed, says how large it is, and closes it. Each is one export here, and the handle between
//! them is opaque — what it holds, and how the child process is kept, is not a C caller's business.
//!
//! # Why the reading is not a callback
//!
//! A reader hands bytes to a callback as they arrive, which would mean calling into another
//! language's thread from a thread this crate started. A poll is the same exchange without that: a
//! thread of this crate drains the pseudo-terminal into a queue, and the caller asks for what is
//! there. Nothing calls back, so nothing has to agree about threads or about a lock the other side
//! is holding.

use std::ffi::{CStr, c_char};
use std::io::{Read, Write};
use std::ptr;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::thread;

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

/// A terminal, as C holds it.
///
/// Opaque: C takes a pointer from `rola_desktop_pty_open` and gives it back with
/// `rola_desktop_pty_close`. A handle that is never closed keeps the shell running.
pub struct RolaDesktopPty {
    /// What the exports share, since C calls them from whichever thread it polls on.
    inner: Mutex<Inner>,
}

/// What one handle holds.
struct Inner {
    /// The master side, which is what a resize is said to.
    master: Option<Box<dyn MasterPty + Send>>,

    /// Where the bytes a user typed go.
    writer: Option<Box<dyn Write + Send>>,

    /// The shell, which is kept so that closing the handle can end it.
    child: Option<Box<dyn Child + Send + Sync>>,

    /// What the reader thread has read and the caller has not asked for yet.
    incoming: Receiver<Vec<u8>>,

    /// The chunk being handed out, and how much of it has been.
    pending: Vec<u8>,
    taken: usize,

    /// Whether the reader thread has stopped, which is the shell having ended.
    ended: bool,
}

/// Opens a terminal and the program in it.
///
/// # FFI
///
/// `program` is the program to run, `arguments` its arguments as a NUL-separated list ended by an
/// empty one (so no arguments is a single NUL), and `directory` where it is run. `cols` and `rows`
/// are the size the view is, which is what the program reads as its window.
///
/// Returns the handle, or null when the terminal or the program could not be started; the caller
/// releases it with `rola_desktop_pty_close`.
///
/// # Safety
///
/// Each pointer must be null or a NUL-terminated UTF-8 string, and `arguments` must be the list the
/// paragraph above describes.
#[allow(nonstandard_style)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rola_desktop_pty_open(
    program: *const c_char,
    arguments: *const c_char,
    directory: *const c_char,
    cols: u16,
    rows: u16,
) -> *mut RolaDesktopPty {
    if program.is_null() {
        return ptr::null_mut();
    }

    // SAFETY: the caller guarantees each pointer is a string of the shape this reads.
    let program = unsafe { text(program) };
    // SAFETY: as above, and the list is read to its double NUL.
    let arguments = unsafe { list(arguments) };

    let mut command = CommandBuilder::new(program);
    command.args(arguments);
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");

    if !directory.is_null() {
        // SAFETY: as above.
        command.cwd(unsafe { text(directory) });
    }

    let Ok(pair) = native_pty_system().openpty(size(cols, rows)) else {
        return ptr::null_mut();
    };

    let Ok(child) = pair.slave.spawn_command(command) else {
        return ptr::null_mut();
    };

    // The slave side is the child's now, and holding it here would keep the terminal open after the
    // shell ends.
    drop(pair.slave);

    let master = pair.master;

    let Ok(reader) = master.try_clone_reader() else {
        return ptr::null_mut();
    };

    let Ok(writer) = master.take_writer() else {
        return ptr::null_mut();
    };

    let (sender, incoming) = channel();

    // The reader is on a thread of its own because reading waits: a caller polling for bytes must not
    // be the thread that waits for them.
    let _ = thread::Builder::new()
        .name("rola-desktop-pty-reader".to_owned())
        .spawn(move || drain(reader, &sender));

    Box::into_raw(Box::new(RolaDesktopPty {
        inner: Mutex::new(Inner {
            master: Some(master),
            writer: Some(writer),
            child: Some(child),
            incoming,
            pending: Vec::new(),
            taken: 0,
            ended: false,
        }),
    }))
}

/// What the shell has printed since the last ask.
///
/// # FFI
///
/// Writes at most `capacity` bytes into `buffer`, and answers how many: zero when nothing has been
/// printed just now (ask again), and `-1` when the shell has ended and nothing is left. The bytes are
/// the terminal's own, escape sequences and all.
///
/// # Safety
///
/// `handle` must be one `rola_desktop_pty_open` handed out and not closed, and `buffer` must be
/// writable for `capacity` bytes.
#[allow(nonstandard_style)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rola_desktop_pty_read(
    handle: *mut RolaDesktopPty,
    buffer: *mut u8,
    capacity: usize,
) -> isize {
    if handle.is_null() || buffer.is_null() || capacity == 0 {
        return 0;
    }

    // SAFETY: the caller guarantees the handle is live and the buffer is writable for `capacity`.
    let Some(pty) = (unsafe { handle.as_ref() }) else {
        return 0;
    };

    let Ok(mut inner) = pty.inner.lock() else {
        return -1;
    };

    // SAFETY: as above.
    let out = unsafe { std::slice::from_raw_parts_mut(buffer, capacity) };

    inner.take(out)
}

/// Sends what a user typed to the shell.
///
/// # FFI
///
/// Answers whether it was written. A shell that has ended takes nothing, which is the false.
///
/// # Safety
///
/// `handle` must be one `rola_desktop_pty_open` handed out and not closed, and `data` must be
/// readable for `length` bytes.
#[allow(nonstandard_style)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rola_desktop_pty_write(
    handle: *mut RolaDesktopPty,
    data: *const u8,
    length: usize,
) -> bool {
    if handle.is_null() || data.is_null() || length == 0 {
        return false;
    }

    // SAFETY: the caller guarantees the handle is live and the bytes are readable for `length`.
    let Some(pty) = (unsafe { handle.as_ref() }) else {
        return false;
    };

    let Ok(mut inner) = pty.inner.lock() else {
        return false;
    };

    let Some(writer) = inner.writer.as_mut() else {
        return false;
    };

    // SAFETY: as above.
    let typed = unsafe { std::slice::from_raw_parts(data, length) };

    writer.write_all(typed).is_ok() && writer.flush().is_ok()
}

/// Says how large the view is, which is what the program lays its screen out by.
///
/// # FFI
///
/// Answers whether it was said. A shell that has ended takes nothing, which is the false.
///
/// # Safety
///
/// `handle` must be one `rola_desktop_pty_open` handed out and not closed.
#[allow(nonstandard_style)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rola_desktop_pty_resize(
    handle: *mut RolaDesktopPty,
    cols: u16,
    rows: u16,
) -> bool {
    if handle.is_null() {
        return false;
    }

    // SAFETY: the caller guarantees the handle is live.
    let Some(pty) = (unsafe { handle.as_ref() }) else {
        return false;
    };

    let Ok(mut inner) = pty.inner.lock() else {
        return false;
    };

    let Some(master) = inner.master.as_mut() else {
        return false;
    };

    master.resize(size(cols, rows)).is_ok()
}

/// Ends the terminal and the shell in it.
///
/// # FFI
///
/// The handle is freed. A null pointer is nothing to close and does nothing.
///
/// # Safety
///
/// `handle` must be one `rola_desktop_pty_open` handed out and not closed, or null.
#[allow(nonstandard_style)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rola_desktop_pty_close(handle: *mut RolaDesktopPty) {
    if handle.is_null() {
        return;
    }

    // SAFETY: the caller guarantees the pointer came from `rola_desktop_pty_open` and was not closed.
    let pty = unsafe { Box::from_raw(handle) };

    let Ok(mut inner) = pty.inner.lock() else {
        return;
    };

    // The writer is taken away first, so that the program sees its input end before it is killed:
    // a shell asked to leave is one that leaves cleanly, and a kill is the answer when it does not.
    inner.writer = None;
    inner.master = None;

    let child = inner.child.take();

    // The reader thread ends with the terminal, and waiting for the child here would wait on the
    // thread that is waiting on the child: it is reaped on a thread of its own.
    let _ = thread::Builder::new()
        .name("rola-desktop-pty-reaper".to_owned())
        .spawn(move || {
            if let Some(mut child) = child {
                let _ = child.kill();
                let _ = child.wait();
            }
        });
}

/// Reads the terminal into the queue until it ends.
///
/// The reader is the master side's own half, and the drop of `sender` — when this thread ends — is how
/// the polling side learns that the shell is over.
fn drain(mut reader: Box<dyn Read + Send>, sender: &std::sync::mpsc::Sender<Vec<u8>>) {
    let mut buffer = [0u8; 8192];

    loop {
        match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => {
                if sender.send(buffer[..read].to_vec()).is_err() {
                    break;
                }
            }
        }
    }
}

impl Inner {
    /// Hands out what has been read, answering zero when nothing has and `-1` at the end.
    ///
    /// One chunk is handed out at a time: a caller with a larger buffer than the chunk gets what there
    /// is and asks again, rather than waiting for the buffer to fill.
    ///
    /// `out` is where the bytes go, and the answer is how many were written, zero for nothing yet, or
    /// `-1` at the end.
    fn take(&mut self, out: &mut [u8]) -> isize {
        loop {
            if self.taken < self.pending.len() {
                let waiting = &self.pending[self.taken..];
                let written = waiting.len().min(out.len());

                out[..written].copy_from_slice(&waiting[..written]);
                self.taken += written;

                if self.taken == self.pending.len() {
                    self.pending.clear();
                    self.taken = 0;
                }

                return isize::try_from(written).unwrap_or(isize::MAX);
            }

            if self.ended {
                return -1;
            }

            match self.incoming.try_recv() {
                Ok(chunk) => {
                    self.pending = chunk;
                    self.taken = 0;
                }
                Err(TryRecvError::Empty) => return 0,
                Err(TryRecvError::Disconnected) => {
                    self.ended = true;

                    return -1;
                }
            }
        }
    }
}

/// Reads a C string, or the empty one for a null pointer.
///
/// The caller guarantees the pointer is NUL-terminated UTF-8.
unsafe fn text(at: *const c_char) -> String {
    if at.is_null() {
        return String::new();
    }

    // SAFETY: the caller guarantees a NUL-terminated string.
    unsafe { CStr::from_ptr(at) }.to_string_lossy().into_owned()
}

/// The arguments a double-NUL-terminated buffer holds.
///
/// The caller guarantees the buffer is NUL-separated and ended by an empty one, which is what says
/// where the list stops without a length being carried beside it.
unsafe fn list(at: *const c_char) -> Vec<String> {
    if at.is_null() {
        return Vec::new();
    }

    let mut arguments = Vec::new();
    let mut cursor = at.cast::<u8>();

    loop {
        let mut length = 0usize;

        // SAFETY: the caller guarantees the buffer is NUL-separated and ended by an empty one, so
        // every byte up to the ending NUL is readable.
        while unsafe { *cursor.add(length) } != 0 {
            length += 1;
        }

        if length == 0 {
            break;
        }

        // SAFETY: as above, the length was just walked.
        let bytes = unsafe { std::slice::from_raw_parts(cursor, length) };
        arguments.push(String::from_utf8_lossy(bytes).into_owned());

        // SAFETY: the walked length is the segment, and the NUL after it is the next segment's start.
        cursor = unsafe { cursor.add(length + 1) };
    }

    arguments
}

/// The size a terminal is told it is.
///
/// No pixel measure crosses, since nothing here draws from it: the program lays its screen out by
/// columns and rows.
const fn size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;
    use std::time::{Duration, Instant};

    use super::{
        rola_desktop_pty_close, rola_desktop_pty_open, rola_desktop_pty_read,
        rola_desktop_pty_write,
    };

    /// A terminal is started, typed into, and read back — which is the whole exchange a view makes.
    ///
    /// The shell is the platform's own, started with no arguments and driven by a line typed into it: what
    /// is checked is that a program in the pseudo-terminal takes its input and writes its answer, not what
    /// a particular shell prints.
    #[test]
    fn a_program_prints_what_it_is_typed() {
        let program = if cfg!(windows) {
            "cmd.exe".to_owned()
        } else {
            "/bin/sh".to_owned()
        };

        let program = CString::new(program).unwrap();
        let directory = CString::new(std::env::temp_dir().to_string_lossy().as_bytes()).unwrap();

        // No arguments is one empty entry, which is the list's terminator on its own.
        let list = [0u8];

        // SAFETY: each pointer is a live, NUL-terminated string of the shape the export reads.
        let handle = unsafe {
            rola_desktop_pty_open(
                program.as_ptr(),
                list.as_ptr().cast(),
                directory.as_ptr(),
                80,
                24,
            )
        };

        assert!(!handle.is_null(), "the terminal could not be opened");

        // The shell draws its prompt once it has one, so the typing waits for it to be up.
        std::thread::sleep(Duration::from_millis(500));

        let typed = b"echo marker\r";
        // SAFETY: the handle is live and the bytes are readable for their length.
        let wrote = unsafe { rola_desktop_pty_write(handle, typed.as_ptr(), typed.len()) };
        assert!(wrote, "the keystrokes were not written");

        let mut seen = Vec::new();
        let mut answered = false;
        let mut buffer = [0u8; 1024];
        let deadline = Instant::now() + Duration::from_secs(20);

        while Instant::now() < deadline {
            // SAFETY: the handle is live and the buffer is writable for its length.
            let read = unsafe { rola_desktop_pty_read(handle, buffer.as_mut_ptr(), buffer.len()) };

            if read > 0 {
                seen.extend_from_slice(&buffer[..read.cast_unsigned()]);

                // A terminal asks where the cursor is and waits for the answer before drawing anything;
                // a caller that does not answer is a caller waiting for a prompt that will not come.
                if !answered && seen.windows(4).any(|window| window == b"\x1b[6n") {
                    let answer = b"\x1b[1;1R";

                    // SAFETY: the handle is live and the bytes are readable for their length.
                    unsafe {
                        rola_desktop_pty_write(handle, answer.as_ptr(), answer.len());
                    }

                    answered = true;
                }

                if String::from_utf8_lossy(&seen).contains("marker") {
                    break;
                }
            } else if read < 0 {
                break;
            }

            std::thread::sleep(Duration::from_millis(10));
        }

        // SAFETY: the handle came from the export above and has not been closed.
        unsafe { rola_desktop_pty_close(handle) };

        let said = String::from_utf8_lossy(&seen);

        assert!(said.contains("marker"), "got {said:?}");
    }
}
