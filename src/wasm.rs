//! WebAssembly entry points for the public website.
//!
//! The browser build runs the whole audit API ([`crate::api`]) inside the visitor's
//! page, so keys never leave their device. The interface is a minimal C ABI (no
//! wasm-bindgen): JS copies UTF-8 strings into memory obtained from
//! [`rsashield_alloc`], calls [`rsashield_call`] with the same method/path/body an
//! HTTP request would carry, then reads the JSON response from
//! [`rsashield_out_ptr`]/[`rsashield_out_len`]. The host must provide the
//! `env.rsashield_now_ms` import (see `crate::clock`). Glue: `web/engine-worker.js`.

use crate::api::{self, AppState};
use std::sync::{Mutex, OnceLock};

static STATE: OnceLock<AppState> = OnceLock::new();
static OUT: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// Allocate `len` bytes for the host to write into.
#[no_mangle]
pub extern "C" fn rsashield_alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::<u8>::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Free memory returned by [`rsashield_alloc`].
///
/// # Safety
/// `ptr` and `len` must come from one earlier `rsashield_alloc(len)` call.
#[no_mangle]
pub unsafe extern "C" fn rsashield_free(ptr: *mut u8, len: usize) {
    drop(Vec::from_raw_parts(ptr, 0, len));
}

unsafe fn text<'a>(ptr: *const u8, len: usize) -> &'a str {
    if len == 0 {
        return "";
    }
    std::str::from_utf8(std::slice::from_raw_parts(ptr, len)).unwrap_or("")
}

/// Handle one API call; returns the HTTP-style status code. The JSON body is left
/// in the output buffer until the next call.
///
/// # Safety
/// Each (ptr, len) pair must describe initialised memory owned by the caller.
#[no_mangle]
pub unsafe extern "C" fn rsashield_call(
    method_ptr: *const u8,
    method_len: usize,
    path_ptr: *const u8,
    path_len: usize,
    body_ptr: *const u8,
    body_len: usize,
) -> u32 {
    let method = text(method_ptr, method_len);
    let path = text(path_ptr, path_len);
    let body = if body_len == 0 { &[][..] } else { std::slice::from_raw_parts(body_ptr, body_len) };
    let state = STATE.get_or_init(AppState::browser);
    let response = api::handle(state, method, path, body);
    let mut out = OUT.lock().expect("output buffer");
    *out = response.body;
    response.status as u32
}

/// Address of the last response body.
#[no_mangle]
pub extern "C" fn rsashield_out_ptr() -> *const u8 {
    OUT.lock().expect("output buffer").as_ptr()
}

/// Length of the last response body in bytes.
#[no_mangle]
pub extern "C" fn rsashield_out_len() -> usize {
    OUT.lock().expect("output buffer").len()
}
