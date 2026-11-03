use core::ffi::c_void;

use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::Controls::{NM_CLICK, NMHDR};

use super::*;
use crate::hwnd::Hwnd;

#[test]
fn decodes_the_raw_header() {
    let hwnd = Hwnd::from_raw(0xC1A1);
    let header = NMHDR {
        hwndFrom: HWND(hwnd.raw() as *mut c_void),
        idFrom: 7,
        code: NM_CLICK,
    };
    let lparam = LPARAM(&header as *const NMHDR as isize);
    let result = decode_notify(lparam);
    assert_eq!(result.id, 7);
    assert_eq!(result.code, NM_CLICK);
    assert_eq!(result.hwnd, hwnd);
}

#[test]
fn a_null_lparam_decodes_to_a_zeroed_notify() {
    let result = decode_notify(LPARAM(0));
    assert_eq!(result.id, 0);
    assert_eq!(result.code, 0);
    assert_eq!(result.hwnd, Hwnd::NULL);
}
