//! Windows part: single instance per user session (named mutex + named pipe), progress
//! TaskDialog with cancel, summary dialog. `HEIC_CONVERT_REPORT=<dir>` runs the same
//! single-instance logic without UI; every serving process writes `<dir>\report-<pid>.txt`
//! (CI checks that a multi-selection ends up in exactly one process).

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use heic_convert::text::{self, Lang};
use heic_convert::{default_workers, Batch};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, ERROR_PIPE_BUSY, HANDLE, HWND, LPARAM, S_FALSE, S_OK, WPARAM,
};
use windows::Win32::Storage::FileSystem::{CreateFileW, PIPE_ACCESS_INBOUND};
use windows::Win32::System::Console::{
    AttachConsole, GetStdHandle, SetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, WaitNamedPipeW, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::{
    CreateMutexW, GetCurrentProcess, SetPriorityClass, BELOW_NORMAL_PRIORITY_CLASS,
};
use windows::Win32::UI::Controls::{
    TaskDialogIndirect, TASKDIALOGCONFIG, TASKDIALOG_NOTIFICATIONS, TDCBF_CANCEL_BUTTON, TDCBF_OK_BUTTON, TDE_CONTENT,
    TDF_ALLOW_DIALOG_CANCELLATION, TDF_CALLBACK_TIMER, TDF_CAN_BE_MINIMIZED, TDF_SHOW_PROGRESS_BAR,
    TDF_SIZE_TO_CONTENT, TDM_CLICK_BUTTON, TDM_SET_ELEMENT_TEXT, TDM_SET_PROGRESS_BAR_POS, TDN_BUTTON_CLICKED,
    TDN_TIMER, TD_INFORMATION_ICON, TD_WARNING_ICON,
};
use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, SendMessageW, IDCANCEL, MB_ICONINFORMATION, MB_OK};
use windows_core::{HRESULT, HSTRING, PCWSTR};

use crate::Args;

/// The batch is closed once it is idle and no new files arrived for this long.
const LINGER: Duration = Duration::from_millis(1500);
/// Explorer starts all processes of a multi-selection within a few seconds at most.
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_LISTED_PROBLEMS: usize = 50;

pub fn lower_priority() {
    unsafe {
        let _ = SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS);
    }
}

/// GUI-subsystem executables have no console; reuse the caller's console for --quiet output.
pub fn attach_parent_console() {
    let missing = |std| unsafe { GetStdHandle(std).map(|h| h.is_invalid() || h.0.is_null()).unwrap_or(true) };
    // Output already redirected (pipe/file): leave it alone.
    if !missing(STD_OUTPUT_HANDLE) {
        return;
    }
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
            return;
        }
        for std in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            if missing(std) {
                if let Ok(h) = CreateFileW(
                    &HSTRING::from("CONOUT$"),
                    (windows::Win32::Foundation::GENERIC_READ | windows::Win32::Foundation::GENERIC_WRITE).0,
                    windows::Win32::Storage::FileSystem::FILE_SHARE_WRITE,
                    None,
                    windows::Win32::Storage::FileSystem::OPEN_EXISTING,
                    windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(0),
                    None,
                ) {
                    let _ = SetStdHandle(std, h);
                }
            }
        }
    }
}

fn instance_names() -> (String, String) {
    let user: String = std::env::var("USERNAME")
        .unwrap_or_default()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let mut session = 0u32;
    unsafe {
        let _ = ProcessIdToSessionId(std::process::id(), &mut session);
    }
    let suffix = std::env::var("HEIC_CONVERT_INSTANCE").unwrap_or_default();
    (
        format!(r"Local\HEICExplorerPreview.Convert.{user}{suffix}"),
        format!(r"\\.\pipe\HEICExplorerPreview.Convert.{session}.{user}{suffix}"),
    )
}

/// Owns the single-instance mutex; dropping/closing it lets the next invocation become a server.
struct InstanceLock(Mutex<Option<isize>>);

impl InstanceLock {
    fn release(&self) {
        if let Some(h) = self.0.lock().unwrap_or_else(|e| e.into_inner()).take() {
            unsafe {
                let _ = CloseHandle(HANDLE(h as *mut _));
            }
        }
    }
}

pub fn run(args: Args) -> i32 {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let (mutex_name, pipe_name) = instance_names();
    let deadline = Instant::now() + CLIENT_TIMEOUT;
    let lock = loop {
        let handle = unsafe { CreateMutexW(None, true, &HSTRING::from(&mutex_name)) };
        let existed = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        match handle {
            Ok(h) if !existed => break Some(h),
            Ok(h) => unsafe {
                let _ = CloseHandle(h);
            },
            Err(_) => break None,
        }
        if send_to_server(&pipe_name, &args.inputs) {
            return 0;
        }
        if Instant::now() > deadline {
            break None; // run on our own rather than lose the files
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let lock = Arc::new(InstanceLock(Mutex::new(lock.map(|h| h.0 as isize))));
    serve(args, pipe_name, lock)
}

fn send_to_server(pipe_name: &str, inputs: &[PathBuf]) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        match std::fs::OpenOptions::new().write(true).open(pipe_name) {
            Ok(mut pipe) => {
                let mut msg = String::new();
                for p in inputs {
                    msg.push_str(&p.to_string_lossy());
                    msg.push('\n');
                }
                return pipe.write_all(msg.as_bytes()).is_ok();
            }
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY.0 as i32) => unsafe {
                let _ = WaitNamedPipeW(&HSTRING::from(pipe_name), 500);
            },
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    false
}

struct Server {
    batch: Arc<Batch>,
    lock: Arc<InstanceLock>,
    last_arrival: Mutex<Instant>,
    lang: Lang,
}

impl Server {
    /// Closes the batch once nothing is left to do and no further invocation showed up.
    fn maybe_close(&self) {
        let quiet_for = self.last_arrival.lock().unwrap_or_else(|e| e.into_inner()).elapsed();
        if !self.batch.is_closed() && self.batch.is_idle() && quiet_for > LINGER {
            self.lock.release();
            self.batch.close();
        }
    }
}

fn serve(args: Args, pipe_name: String, lock: Arc<InstanceLock>) -> i32 {
    let batch = Batch::new();
    batch.add(&args.inputs);
    let workers = batch.spawn_workers(default_workers());
    let server = Arc::new(Server {
        batch: batch.clone(),
        lock: lock.clone(),
        last_arrival: Mutex::new(Instant::now()),
        lang: args.lang,
    });
    {
        let server = server.clone();
        std::thread::spawn(move || listen(&pipe_name, &server));
    }

    let code = match std::env::var_os("HEIC_CONVERT_REPORT") {
        Some(report) => headless(&server, PathBuf::from(report)),
        None => interactive(&server),
    };
    lock.release();
    batch.close();
    for w in workers {
        let _ = w.join();
    }
    code
}

fn listen(pipe_name: &str, server: &Server) {
    let name = HSTRING::from(pipe_name);
    loop {
        let pipe = unsafe {
            CreateNamedPipeW(
                &name,
                PIPE_ACCESS_INBOUND,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                0,
                64 * 1024,
                0,
                None,
            )
        };
        if pipe.is_invalid() {
            return;
        }
        // ERROR_PIPE_CONNECTED (client connected before we waited) is fine as well.
        let _ = unsafe { ConnectNamedPipe(pipe, None) };
        let mut data = Vec::new();
        let mut buf = [0u8; 8192];
        loop {
            let mut read = 0u32;
            let ok =
                unsafe { windows::Win32::Storage::FileSystem::ReadFile(pipe, Some(&mut buf), Some(&mut read), None) };
            if ok.is_err() || read == 0 {
                break;
            }
            data.extend_from_slice(&buf[..read as usize]);
        }
        unsafe {
            let _ = DisconnectNamedPipe(pipe);
            let _ = CloseHandle(pipe);
        }
        let paths: Vec<PathBuf> =
            String::from_utf8_lossy(&data).lines().filter(|l| !l.is_empty()).map(PathBuf::from).collect();
        if paths.is_empty() {
            continue;
        }
        if server.batch.is_closed() {
            // Arrived just after we stopped accepting: hand over to a fresh instance.
            if let Ok(exe) = std::env::current_exe() {
                let lang = if server.lang == Lang::De { "--lang=de" } else { "--lang=en" };
                let _ = std::process::Command::new(exe).arg(lang).args(&paths).spawn();
            }
        } else {
            *server.last_arrival.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
            server.batch.add(&paths);
        }
    }
}

fn headless(server: &Server, report: PathBuf) -> i32 {
    loop {
        server.maybe_close();
        if server.batch.progress().done {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let p = server.batch.progress();
    let mut text = format!(
        "pid={}\ntotal={}\nconverted={}\nfailed={}\nskipped={}\n",
        std::process::id(),
        p.total,
        p.converted,
        p.failed,
        p.skipped
    );
    for line in text::problem_lines(server.lang, &server.batch.results()) {
        text.push_str(&line);
        text.push('\n');
    }
    let _ = std::fs::write(report.join(format!("report-{}.txt", std::process::id())), text);
    i32::from(p.failed > 0)
}

struct DialogState<'a> {
    server: &'a Server,
    finishing: AtomicBool,
}

unsafe extern "system" fn progress_callback(
    hwnd: HWND,
    msg: TASKDIALOG_NOTIFICATIONS,
    wparam: WPARAM,
    _lparam: LPARAM,
    data: isize,
) -> HRESULT {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let state = unsafe { &*(data as *const DialogState) };
        let server = state.server;
        match msg {
            TDN_TIMER => {
                server.maybe_close();
                let p = server.batch.progress();
                let pos = (p.finished * 100).checked_div(p.total).unwrap_or(0);
                let line = if p.cancelled {
                    HSTRING::from(text::cancelling(server.lang))
                } else {
                    HSTRING::from(text::progress_line(server.lang, &p))
                };
                unsafe {
                    SendMessageW(hwnd, TDM_SET_PROGRESS_BAR_POS.0 as u32, Some(WPARAM(pos)), None);
                    SendMessageW(
                        hwnd,
                        TDM_SET_ELEMENT_TEXT.0 as u32,
                        Some(WPARAM(TDE_CONTENT.0 as usize)),
                        Some(LPARAM(line.as_ptr() as isize)),
                    );
                }
                if p.done && !state.finishing.swap(true, Ordering::SeqCst) {
                    unsafe {
                        SendMessageW(hwnd, TDM_CLICK_BUTTON.0 as u32, Some(WPARAM(IDCANCEL.0 as usize)), None);
                    }
                }
                S_OK
            }
            TDN_BUTTON_CLICKED if wparam.0 == IDCANCEL.0 as usize => {
                if state.finishing.load(Ordering::SeqCst) {
                    S_OK
                } else {
                    // User cancel: stop queued files, keep the dialog until running ones are written.
                    server.lock.release();
                    server.batch.cancel();
                    S_FALSE
                }
            }
            _ => S_OK,
        }
    }));
    result.unwrap_or(S_OK)
}

fn interactive(server: &Server) -> i32 {
    let lang = server.lang;
    let title = HSTRING::from(text::title(lang));
    let heading = HSTRING::from(text::working(lang));
    let initial = HSTRING::from(text::progress_line(lang, &server.batch.progress()));
    let state = DialogState { server, finishing: AtomicBool::new(false) };
    let config = TASKDIALOGCONFIG {
        cbSize: std::mem::size_of::<TASKDIALOGCONFIG>() as u32,
        dwFlags: TDF_SHOW_PROGRESS_BAR
            | TDF_CALLBACK_TIMER
            | TDF_ALLOW_DIALOG_CANCELLATION
            | TDF_CAN_BE_MINIMIZED
            | TDF_SIZE_TO_CONTENT,
        dwCommonButtons: TDCBF_CANCEL_BUTTON,
        pszWindowTitle: PCWSTR(title.as_ptr()),
        pszMainInstruction: PCWSTR(heading.as_ptr()),
        pszContent: PCWSTR(initial.as_ptr()),
        pfCallback: Some(progress_callback),
        lpCallbackData: &state as *const DialogState as isize,
        ..Default::default()
    };
    let shown = unsafe { TaskDialogIndirect(&config, None, None, None) }.is_ok();
    if !shown {
        // No common controls v6 (should not happen with the embedded manifest): just wait.
        loop {
            server.maybe_close();
            if server.batch.progress().done {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    server.batch.wait_done();
    show_summary(server);
    i32::from(server.batch.progress().failed > 0)
}

fn show_summary(server: &Server) {
    let lang = server.lang;
    let p = server.batch.progress();
    let problems = text::problem_lines(lang, &server.batch.results());
    let mut details = problems.iter().take(MAX_LISTED_PROBLEMS).cloned().collect::<Vec<_>>().join("\n");
    if problems.len() > MAX_LISTED_PROBLEMS {
        details.push_str(&format!("\n… (+{})", problems.len() - MAX_LISTED_PROBLEMS));
    }
    let title = HSTRING::from(text::title(lang));
    let heading = HSTRING::from(text::summary_heading(lang, &p));
    let content = HSTRING::from(text::summary_details(lang, &p));
    let expanded = HSTRING::from(details.as_str());
    let mut config = TASKDIALOGCONFIG {
        cbSize: std::mem::size_of::<TASKDIALOGCONFIG>() as u32,
        dwFlags: TDF_ALLOW_DIALOG_CANCELLATION | TDF_SIZE_TO_CONTENT,
        dwCommonButtons: TDCBF_OK_BUTTON,
        pszWindowTitle: PCWSTR(title.as_ptr()),
        pszMainInstruction: PCWSTR(heading.as_ptr()),
        pszContent: PCWSTR(content.as_ptr()),
        ..Default::default()
    };
    config.Anonymous1.pszMainIcon = if p.failed > 0 || p.total == 0 { TD_WARNING_ICON } else { TD_INFORMATION_ICON };
    if !problems.is_empty() {
        config.pszExpandedInformation = PCWSTR(expanded.as_ptr());
    }
    if unsafe { TaskDialogIndirect(&config, None, None, None) }.is_err() {
        let body = HSTRING::from(format!(
            "{}\n\n{}\n\n{}",
            text::summary_heading(lang, &p),
            text::summary_details(lang, &p),
            details
        ));
        unsafe {
            MessageBoxW(None, &body, &title, MB_OK | MB_ICONINFORMATION);
        }
    }
}
