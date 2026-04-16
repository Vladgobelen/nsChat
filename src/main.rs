#![cfg_attr(not(windows), allow(dead_code))]

#[cfg(windows)]
extern crate winapi;

#[cfg(windows)]
use winapi::um::{
    winuser::*,
    winbase::*,
    namedpipeapi::*,
    handleapi::*,
    processthreadsapi::*,
    libloaderapi::*,
    errhandlingapi::*,
    fileapi::*,
    winnt::HANDLE,
};
#[cfg(windows)]
use winapi::shared::{
    minwindef::*,
    windef::HWND,
    ntdef::NULL,
    winerror::ERROR_PIPE_CONNECTED,
};
#[cfg(windows)]
use std::ffi::OsStr;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use std::sync::Mutex;
#[cfg(windows)]
use std::thread;
#[cfg(windows)]
use std::ptr::null_mut;
#[cfg(windows)]
use serde::{Deserialize, Serialize};

#[cfg(windows)]
const WINDOW_CLASS_NAME: &str = "NSQCuE_Overlay_Window";
#[cfg(windows)]
const PIPE_NAME: &str = r"\\.\pipe\NSQCuE_Overlay_Pipe";
#[cfg(windows)]
const WM_USER_ADD_MESSAGE: u32 = WM_USER + 1;

#[cfg(windows)]
struct UnsafeSend<T>(T);
#[cfg(windows)]
unsafe impl<T> Send for UnsafeSend<T> {}

#[cfg(windows)]
static MESSAGES_LIST: Mutex<Option<UnsafeSend<HWND>>> = Mutex::new(None);
#[cfg(windows)]
static INPUT_FIELD: Mutex<Option<UnsafeSend<HWND>>> = Mutex::new(None);
#[cfg(windows)]
static PIPE_HANDLE: Mutex<Option<UnsafeSend<HANDLE>>> = Mutex::new(None);

#[cfg(windows)]
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum PipeMessage {
    #[serde(rename = "message")]
    Message { text: String },
    #[serde(rename = "input")]
    Input { text: String },
}

#[cfg(not(windows))]
fn main() {
    println!("This application only runs on Windows.");
}

#[cfg(windows)]
fn main() {
    println!("Chat Overlay starting...");
    
    let pipe_thread = thread::spawn(|| {
        pipe_server_thread();
    });
    
    create_window();
    pipe_thread.join().unwrap();
}

#[cfg(windows)]
fn create_window() {
    unsafe {
        let hinstance = GetModuleHandleW(null_mut());
        
        let class_name: Vec<u16> = OsStr::new(WINDOW_CLASS_NAME)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_WINDOW + 1) as HBRUSH,
            lpszMenuName: null_mut(),
            lpszClassName: class_name.as_ptr(),
        };
        
        RegisterClassW(&wc);
        
        let title: Vec<u16> = OsStr::new("Chat Overlay")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_POPUP | WS_VISIBLE | WS_THICKFRAME,
            100, 100, 400, 500,
            null_mut(),
            null_mut(),
            hinstance,
            null_mut(),
        );
        
        SetLayeredWindowAttributes(hwnd, 0, 240, LWA_ALPHA);
        create_controls(hwnd);
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
        
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

#[cfg(windows)]
fn create_controls(parent: HWND) {
    unsafe {
        let hinstance = GetModuleHandleW(null_mut());
        
        let list_hwnd = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            "LISTBOX\0".as_ptr() as *const u16,
            null_mut(),
            WS_CHILD | WS_VISIBLE | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT,
            10, 10, 380, 380,
            parent,
            null_mut(),
            hinstance,
            null_mut(),
        );
        
        *MESSAGES_LIST.lock().unwrap() = Some(UnsafeSend(list_hwnd));
        
        let input_hwnd = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            "EDIT\0".as_ptr() as *const u16,
            null_mut(),
            WS_CHILD | WS_VISIBLE | ES_AUTOHSCROLL,
            10, 400, 300, 25,
            parent,
            null_mut(),
            hinstance,
            null_mut(),
        );
        
        *INPUT_FIELD.lock().unwrap() = Some(UnsafeSend(input_hwnd));
        
        CreateWindowExW(
            0,
            "BUTTON\0".as_ptr() as *const u16,
            "Send\0".as_ptr() as *const u16,
            WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
            320, 400, 70, 25,
            parent,
            1 as HMENU,
            hinstance,
            null_mut(),
        );
    }
}

#[cfg(windows)]
unsafe extern "system" fn wndproc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => 0,
        
        WM_NCHITTEST => {
            let result = DefWindowProcW(hwnd, msg, wparam, lparam);
            if result == HTCLIENT as LRESULT {
                return HTCAPTION as LRESULT;
            }
            result
        }
        
        WM_COMMAND => {
            let code = HIWORD(wparam as u32);
            let id = LOWORD(wparam as u32) as i32;
            
            if id == 1 && code == BN_CLICKED {
                send_input_to_pipe();
            }
            0
        }
        
        WM_KEYDOWN => {
            if wparam as i32 == 13 {
                if let Some(UnsafeSend(input_hwnd)) = *INPUT_FIELD.lock().unwrap() {
                    if GetFocus() == input_hwnd {
                        send_input_to_pipe();
                        return 0;
                    }
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        
        WM_USER_ADD_MESSAGE => {
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(
                lparam as *const u16,
                wparam as usize,
            ));
            add_message_to_list(&text);
            0
        }
        
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(windows)]
fn send_input_to_pipe() {
    unsafe {
        if let Some(UnsafeSend(input_hwnd)) = *INPUT_FIELD.lock().unwrap() {
            let mut buffer = vec![0u16; 1024];
            let len = GetWindowTextW(input_hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
            
            if len > 0 {
                let text = String::from_utf16_lossy(&buffer[..len as usize]);
                SetWindowTextW(input_hwnd, null_mut());
                
                let msg = PipeMessage::Input { text };
                if let Ok(json) = serde_json::to_string(&msg) {
                    send_to_pipe(&json);
                }
            }
        }
    }
}

#[cfg(windows)]
fn add_message_to_list(text: &str) {
    unsafe {
        if let Some(UnsafeSend(list_hwnd)) = *MESSAGES_LIST.lock().unwrap() {
            let wide_text: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            SendMessageW(list_hwnd, LB_ADDSTRING, 0, wide_text.as_ptr() as LPARAM);
            
            let count = SendMessageW(list_hwnd, LB_GETCOUNT, 0, 0);
            SendMessageW(list_hwnd, LB_SETTOPINDEX, (count - 1) as WPARAM, 0);
        }
    }
}

#[cfg(windows)]
fn pipe_server_thread() {
    loop {
        unsafe {
            let pipe_name: Vec<u16> = OsStr::new(PIPE_NAME)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            
            let pipe_handle = CreateNamedPipeW(
                pipe_name.as_ptr(),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
                1,
                4096,
                4096,
                0,
                null_mut(),
            );
            
            if pipe_handle == INVALID_HANDLE_VALUE {
                thread::sleep(std::time::Duration::from_secs(1));
                continue;
            }
            
            *PIPE_HANDLE.lock().unwrap() = Some(UnsafeSend(pipe_handle));
            
            let connected = ConnectNamedPipe(pipe_handle, null_mut()) != 0;
            if !connected && GetLastError() != ERROR_PIPE_CONNECTED {
                CloseHandle(pipe_handle);
                continue;
            }
            
            let mut buffer = vec![0u8; 4096];
            let mut bytes_read = 0u32;
            
            loop {
                let result = ReadFile(
                    pipe_handle,
                    buffer.as_mut_ptr() as *mut _,
                    buffer.len() as u32,
                    &mut bytes_read,
                    null_mut(),
                );
                
                if result != 0 && bytes_read > 0 {
                    if let Ok(json_str) = std::str::from_utf8(&buffer[..bytes_read as usize]) {
                        if let Ok(PipeMessage::Message { text }) = serde_json::from_str(json_str) {
                            let wide_text: Vec<u16> = text.encode_utf16().collect();
                            if let Some(hwnd) = find_overlay_window() {
                                PostMessageW(
                                    hwnd,
                                    WM_USER_ADD_MESSAGE,
                                    wide_text.len() as WPARAM,
                                    wide_text.as_ptr() as LPARAM,
                                );
                            }
                        }
                    }
                } else {
                    break;
                }
            }
            
            DisconnectNamedPipe(pipe_handle);
            CloseHandle(pipe_handle);
            *PIPE_HANDLE.lock().unwrap() = None;
        }
    }
}

#[cfg(windows)]
fn send_to_pipe(data: &str) {
    unsafe {
        if let Some(UnsafeSend(pipe)) = *PIPE_HANDLE.lock().unwrap() {
            let bytes = data.as_bytes();
            let mut bytes_written = 0u32;
            
            WriteFile(
                pipe,
                bytes.as_ptr() as *const _,
                bytes.len() as u32,
                &mut bytes_written,
                null_mut(),
            );
            
            FlushFileBuffers(pipe);
        }
    }
}

#[cfg(windows)]
fn find_overlay_window() -> Option<HWND> {
    unsafe {
        let class_name: Vec<u16> = OsStr::new(WINDOW_CLASS_NAME)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        
        let hwnd = FindWindowW(class_name.as_ptr(), null_mut());
        if hwnd != null_mut() {
            Some(hwnd)
        } else {
            None
        }
    }
}

#[cfg(windows)]
fn LOWORD(l: u32) -> u16 {
    (l & 0xFFFF) as u16
}

#[cfg(windows)]
fn HIWORD(l: u32) -> u16 {
    ((l >> 16) & 0xFFFF) as u16
}