//! External-process MiniDumpNormal with bounded alternate I/O. The API receives
//! a read-only file handle: an unsupported/omitted IoStart cannot bypass the cap.
use crate::limited_writer::LimitedWriter;
use std::{
    fs::File,
    io::{self, Seek, SeekFrom, Write},
    os::windows::io::AsRawHandle,
    sync::Mutex,
};
use windows::{
    Win32::{
        Foundation::{BOOL, CloseHandle, HANDLE},
        System::{
            Diagnostics::Debug::{
                IoFinishCallback, IoStartCallback, IoWriteAllCallback,
                MINIDUMP_CALLBACK_INFORMATION, MINIDUMP_CALLBACK_INPUT, MINIDUMP_CALLBACK_OUTPUT,
                MINIDUMP_EXCEPTION_INFORMATION, MiniDumpNormal, MiniDumpWriteDump,
            },
            Threading::{OpenProcess, PROCESS_ALL_ACCESS},
        },
    },
    core::HRESULT,
};

struct Process(HANDLE);
impl Drop for Process {
    fn drop(&mut self) {
        // SAFETY: exclusively owns this OpenProcess handle, not a pseudo handle.
        let _ = unsafe { CloseHandle(self.0) };
    }
}
struct State<'a> {
    writer: LimitedWriter<&'a mut File>,
    limit: u64,
    started: bool,
    finished: bool,
    failed: bool,
}

// SAFETY: DbgHelp supplies readable input/writable output for each synchronous
// callback; context points to a live Mutex<State> until MiniDumpWriteDump returns.
// The mutex serializes callback file positions even if DbgHelp changes threads.
// No panicking operations, formatting or unbounded buffers cross this FFI edge.
unsafe extern "system" fn callback(
    context: *mut core::ffi::c_void,
    input: *const MINIDUMP_CALLBACK_INPUT,
    output: *mut MINIDUMP_CALLBACK_OUTPUT,
) -> BOOL {
    if context.is_null() || input.is_null() || output.is_null() {
        return BOOL(0);
    }
    // Bindings are packed(4); copy fields instead of creating unaligned refs.
    let input = unsafe { input.read_unaligned() };
    let kind = input.CallbackType;
    if kind != IoStartCallback.0 as u32
        && kind != IoWriteAllCallback.0 as u32
        && kind != IoFinishCallback.0 as u32
    {
        // Preserve DbgHelp's existing thread/module flags and all default output.
        return BOOL(1);
    }
    let state = unsafe { &*context.cast::<Mutex<State<'_>>>() };
    let Ok(mut state) = state.lock() else {
        return BOOL(0);
    };
    let mut status = HRESULT(0);
    let ok = if state.failed {
        false
    } else if kind == IoStartCallback.0 as u32 {
        state.started = true;
        status = HRESULT(1); // S_FALSE selects alternate I/O for every write.
        true
    } else if kind == IoWriteAllCallback.0 as u32 {
        let request = unsafe { input.Anonymous.Io };
        let offset = request.Offset;
        let size = request.BufferBytes as usize;
        if !state.started
            || state.finished
            || offset
                .checked_add(size as u64)
                .is_none_or(|n| n > state.limit)
            || (size != 0 && request.Buffer.is_null())
        {
            false
        } else {
            let bytes = if size == 0 {
                &[]
            } else {
                // SAFETY: DbgHelp owns a readable BufferBytes buffer for this call.
                // Range is checked first; no allocation or copy of the payload.
                unsafe { std::slice::from_raw_parts(request.Buffer.cast::<u8>(), size) }
            };
            state
                .writer
                .seek(SeekFrom::Start(offset))
                .and_then(|_| state.writer.write_all(bytes))
                .is_ok()
        }
    } else {
        state.finished = true;
        state.started && state.writer.check().is_ok()
    };
    if !ok {
        state.failed = true;
        status = HRESULT(0x80004005_u32 as i32);
    }
    // SAFETY: output is writable but packed; touch only the I/O union member.
    unsafe {
        core::ptr::addr_of_mut!((*output).Anonymous.Status).write_unaligned(status);
    }
    BOOL(i32::from(ok))
}

pub(crate) fn dump(
    context: &crash_context::CrashContext,
    file: &mut File,
    readonly: &File,
    limit: u64,
) -> io::Result<()> {
    if context.process_id == std::process::id() {
        return Err(io::Error::other(
            "Bounded dump requires an external process",
        ));
    }
    // SAFETY: same access and external exception-pointer contract as upstream.
    // The parent remains stopped/alive until the existing IPC request returns.
    let process = Process(
        unsafe { OpenProcess(PROCESS_ALL_ACCESS, false, context.process_id) }
            .map_err(io::Error::other)?,
    );
    let info = MINIDUMP_EXCEPTION_INFORMATION {
        ThreadId: context.thread_id,
        ExceptionPointers: context.exception_pointers.cast_mut().cast(),
        ClientPointers: BOOL(1),
    };
    let state = Mutex::new(State {
        writer: LimitedWriter::new(file, limit)?,
        limit,
        started: false,
        finished: false,
        failed: false,
    });
    let callbacks = MINIDUMP_CALLBACK_INFORMATION {
        CallbackRoutine: Some(callback),
        CallbackParam: (&state as *const Mutex<State<'_>>).cast_mut().cast(),
    };
    // SAFETY: all borrowed state and exception information remain valid until
    // this synchronous call returns. The read-only handle cannot grow the file.
    let result = unsafe {
        MiniDumpWriteDump(
            process.0,
            context.process_id,
            HANDLE(readonly.as_raw_handle() as isize),
            MiniDumpNormal,
            (!context.exception_pointers.is_null()).then_some(&info),
            None,
            Some(&callbacks),
        )
    };
    let state = state
        .into_inner()
        .map_err(|_| io::Error::other("Dump I/O state unavailable"))?;
    state.writer.check()?;
    if state.failed || !state.started || !state.finished {
        return Err(io::Error::other("Bounded dump I/O incomplete"));
    }
    result.map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::Diagnostics::Debug::{
        MINIDUMP_CALLBACK_INPUT_0, MINIDUMP_IO_CALLBACK, ModuleCallback,
    };

    fn invoke(
        state: &Mutex<State<'_>>,
        kind: u32,
        offset: u64,
        bytes: &[u8],
        output: &mut MINIDUMP_CALLBACK_OUTPUT,
    ) -> BOOL {
        let input = MINIDUMP_CALLBACK_INPUT {
            CallbackType: kind,
            Anonymous: MINIDUMP_CALLBACK_INPUT_0 {
                Io: MINIDUMP_IO_CALLBACK {
                    Offset: offset,
                    Buffer: bytes.as_ptr().cast_mut().cast(),
                    BufferBytes: bytes.len() as u32,
                    ..Default::default()
                },
            },
            ..Default::default()
        };
        // SAFETY: all buffers/state/output remain live for this synchronous call.
        unsafe {
            callback(
                (state as *const Mutex<State<'_>>).cast_mut().cast(),
                &input,
                output,
            )
        }
    }

    #[test]
    fn callback_caps_offsets_latches_failures_and_preserves_non_io_flags() {
        let mut file = tempfile::tempfile().unwrap();
        {
            let state = Mutex::new(State {
                writer: LimitedWriter::new(&mut file, 8).unwrap(),
                limit: 8,
                started: false,
                finished: false,
                failed: false,
            });
            let mut output = MINIDUMP_CALLBACK_OUTPUT::default();
            output.Anonymous.ModuleWriteFlags = 0x123;
            assert!(invoke(&state, ModuleCallback.0 as u32, 0, &[], &mut output).as_bool());
            // SAFETY: the previous callback preserved the initialized union member.
            assert_eq!(unsafe { output.Anonymous.ModuleWriteFlags }, 0x123);
            assert!(invoke(&state, IoStartCallback.0 as u32, 0, &[], &mut output).as_bool());
            // SAFETY: IoStart writes Status.
            assert_eq!(unsafe { output.Anonymous.Status }, HRESULT(1));
            assert!(
                invoke(
                    &state,
                    IoWriteAllCallback.0 as u32,
                    0,
                    b"12345678",
                    &mut output
                )
                .as_bool()
            );
            assert!(invoke(&state, IoWriteAllCallback.0 as u32, 0, b"HEAD", &mut output).as_bool());
            assert!(
                !invoke(
                    &state,
                    IoWriteAllCallback.0 as u32,
                    u64::MAX,
                    b"x",
                    &mut output
                )
                .as_bool()
            );
            assert!(!invoke(&state, IoFinishCallback.0 as u32, 0, &[], &mut output).as_bool());
        }
        assert_eq!(file.metadata().unwrap().len(), 8);
        file.seek(SeekFrom::Start(0)).unwrap();
        let mut data = Vec::new();
        std::io::Read::read_to_end(&mut file, &mut data).unwrap();
        assert_eq!(data, b"HEAD5678");
    }

    #[test]
    fn readonly_native_fallback_handle_cannot_write() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        let mut readonly = File::open(temp.path()).unwrap();
        assert!(readonly.write_all(b"must not grow").is_err());
        assert_eq!(temp.as_file().metadata().unwrap().len(), 0);
    }
}
