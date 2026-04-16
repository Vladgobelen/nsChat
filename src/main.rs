#![cfg_attr(not(windows), allow(dead_code))]

#[cfg(windows)]
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::System::Pipes::*,
    Win32::System::Threading::*,
    Win32::System::LibraryLoader::GetModuleHandleW,
    Win32::UI::Controls::*,
    Win32::UI::WindowsAndMessaging::*,
};

#[cfg(windows)]
use std::io::Write;
#[cfg(windows)]
use std::sync::Mutex;
#[cfg(windows)]
use std::thread;
#[cfg(windows)]
use serde::{Deserialize, Serialize};

#[cfg(windows)]
const WINDOW_CLASS_NAME: &str = "NSQCuE_Overlay_Window";
#[cfg(windows)]
const PIPE_NAME: &str = r"\\.\pipe\NSQCuE_Overlay_Pipe";
#[cfg(windows)]
const WM_USER_ADD_MESSAGE: u32 = WM_USER + 1;

#[cfg(windows)]
static MESSAGES_LIST: Mutex<Option<HWND>> = Mutex::new(None);
#[cfg(windows)]
static INPUT_FIELD: Mutex<Option<HWND>> = Mutex::new(None);
#[cfg(windows)]
static PIPE_HANDLE: Mutex<Option<HANDLE>> = Mutex::new(None);

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
fn main() -> Result<()> {
    println!("Chat Overlay starting...");
    
    let pipe_thread = thread::spawn(|| {
        pipe_server_thread();
    });
    
    create_window()?;
    pipe_thread.join().unwrap();
    
    Ok(())
}

#[cfg(windows)]
fn create_window() -> Result<()> {
    unsafe {
        let hinstance = GetModuleHandleW(None)?;
        
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance,
            lpszClassName: HSTRING::from(WINDOW_CLASS_NAME).as_wide(),
            ..Default::default()
        };
        
        if RegisterClassW(&wc) == 0 {
            return Err(Error::from_win32());
        }
        
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
            HSTRING::from(WINDOW_CLASS_NAME).as_wide(),
            HSTRING::from("Chat Overlay").as_wide(),
            WS_POPUP | WS_VISIBLE | WS_THICKFRAME,
            100, 100, 400, 500,
            None,
            None,
            hinstance,
            None,
        );
        
        if hwnd.0 == 0 {
            return Err(Error::from_win32());
        }
        
        SetLayeredWindowAttributes(hwnd, 0, 240, LWA_ALPHA);
        create_controls(hwnd)?;
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
        
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        
        Ok(())
    }
}

#[cfg(windows)]
fn create_controls(parent: HWND) -> Result<()> {
    unsafe {
        let hinstance = GetModuleHandleW(None)?;
        
        let list_hwnd = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            HSTRING::from("LISTBOX").as_wide(),
            HSTRING::from("").as_wide(),
            WS_CHILD | WS_VISIBLE | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT,
            10, 10, 380, 380,
            parent,
            None,
            hinstance,
            None,
        );
        
        if list_hwnd.0 == 0 {
            return Err(Error::from_win32());
        }
        *MESSAGES_LIST.lock().unwrap() = Some(list_hwnd);
        
        let input_hwnd = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            HSTRING::from("EDIT").as_wide(),
            HSTRING::from("").as_wide(),
            WS_CHILD | WS_VISIBLE | ES_AUTOHSCROLL,
            10, 400, 300, 25,
            parent,
            None,
            hinstance,
            None,
        );
        
        if input_hwnd.0 == 0 {
            return Err(Error::from_win32());
        }
        *INPUT_FIELD.lock().unwrap() = Some(input_hwnd);
        
        let button_hwnd = CreateWindowExW(
            0,
            HSTRING::from("BUTTON").as_wide(),
            HSTRING::from("Send").as_wide(),
            WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
            320, 400, 70, 25,
            parent,
            None,
            hinstance,
            None,
        );
        
        if button_hwnd.0 == 0 {
            return Err(Error::from_win32());
        }
        
        Ok(())
    }
}

#[cfg(windows)]
extern "system" fn wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                EnableWindow(hwnd, BOOL::from(true));
                LRESULT(0)
            }
            
            WM_NCHITTEST => {
                let result = DefWindowProcW(hwnd, msg, wparam, lparam);
                if result.0 == HTCLIENT as isize {
                    LRESULT(HTCAPTION as isize)
                } else {
                    result
                }
            }
            
            WM_COMMAND => {
                let notification = HIWORD(wparam.0 as u32);
                if notification == BN_CLICKED as u16 {
                    send_input_to_pipe();
                }
                LRESULT(0)
            }
            
            WM_KEYDOWN => {
                if wparam.0 == 13 {
                    let input_hwnd = INPUT_FIELD.lock().unwrap().unwrap();
                    if GetFocus() == input_hwnd {
                        send_input_to_pipe();
                        return LRESULT(0);
                    }
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            
            WM_USER_ADD_MESSAGE => {
                let text_ptr = lparam.0 as *const u16;
                let text = String::from_utf16_lossy(std::slice::from_raw_parts(
                    text_ptr,
                    wparam.0 as usize,
                ));
                add_message_to_list(&text);
                LRESULT(0)
            }
            
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

#[cfg(windows)]
fn send_input_to_pipe() {
    unsafe {
        let input_hwnd = INPUT_FIELD.lock().unwrap().unwrap();
        let mut buffer = vec![0u16; 1024];
        let len = GetWindowTextW(input_hwnd, &mut buffer);
        
        if len > 0 {
            let text = String::from_utf16_lossy(&buffer[..len as usize]);
            SetWindowTextW(input_hwnd, HSTRING::from("").as_wide());
            
            let msg = PipeMessage::Input { text };
            if let Ok(json) = serde_json::to_string(&msg) {
                send_to_pipe(&json);
            }
        }
    }
}

#[cfg(windows)]
fn add_message_to_list(text: &str) {
    unsafe {
        if let Some(list_hwnd) = *MESSAGES_LIST.lock().unwrap() {
            let wide_text: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            SendMessageW(list_hwnd, LB_ADDSTRING, WPARAM(0), LPARAM(wide_text.as_ptr() as isize));
            
            let count = SendMessageW(list_hwnd, LB_GETCOUNT, WPARAM(0), LPARAM(0));
            SendMessageW(list_hwnd, LB_SETTOPINDEX, WPARAM(count.0 as usize - 1), LPARAM(0));
        }
    }
}

#[cfg(windows)]
fn pipe_server_thread() {
    loop {
        unsafe {
            let pipe_handle = CreateNamedPipeW(
                HSTRING::from(PIPE_NAME).as_wide(),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
                1,
                4096,
                4096,
                0,
                None,
            );
            
            if pipe_handle.is_invalid() {
                thread::sleep(std::time::Duration::from_secs(1));
                continue;
            }
            
            *PIPE_HANDLE.lock().unwrap() = Some(pipe_handle);
            
            let connected = ConnectNamedPipe(pipe_handle, None).as_bool();
            if !connected && GetLastError() != ERROR_PIPE_CONNECTED {
                CloseHandle(pipe_handle);
                continue;
            }
            
            let mut buffer = vec![0u8; 4096];
            let mut bytes_read = 0u32;
            
            loop {
                let result = ReadFile(
                    pipe_handle,
                    Some(buffer.as_mut_slice()),
                    Some(&mut bytes_read),
                    None,
                );
                
                if result.is_ok() && bytes_read > 0 {
                    if let Ok(json_str) = std::str::from_utf8(&buffer[..bytes_read as usize]) {
                        if let Ok(PipeMessage::Message { text }) = serde_json::from_str(json_str) {
                            let wide_text: Vec<u16> = text.encode_utf16().collect();
                            if let Some(hwnd) = find_overlay_window() {
                                PostMessageW(
                                    hwnd,
                                    WM_USER_ADD_MESSAGE,
                                    WPARAM(wide_text.len()),
                                    LPARAM(wide_text.as_ptr() as isize),
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
        if let Some(pipe) = *PIPE_HANDLE.lock().unwrap() {
            let bytes = data.as_bytes();
            let mut bytes_written = 0u32;
            
            let _ = WriteFile(
                pipe,
                Some(bytes),
                Some(&mut bytes_written),
                None,
            );
            FlushFileBuffers(pipe);
        }
    }
}

#[cfg(windows)]
fn find_overlay_window() -> Option<HWND> {
    unsafe {
        let hwnd = FindWindowW(HSTRING::from(WINDOW_CLASS_NAME).as_wide(), None);
        if hwnd.0 != 0 { Some(hwnd) } else { None }
    }
}