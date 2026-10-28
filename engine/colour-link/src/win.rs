// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, the Visual Pinball Infinite contributors

//! The few Windows calls the link needs (kernel32), declared from
//! Microsoft's documentation.

#![allow(non_snake_case)]

use std::ffi::c_void;

pub type Handle = *mut c_void;

pub const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;
pub const PAGE_READWRITE: u32 = 0x04;
pub const FILE_MAP_ALL_ACCESS: u32 = 0x000F_001F;
pub const EVENT_MODIFY_STATE: u32 = 0x0002;
pub const SYNCHRONIZE: u32 = 0x0010_0000;
pub const ERROR_ALREADY_EXISTS: i32 = 183;
pub const WAIT_OBJECT_0: u32 = 0;

#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn CreateFileMappingW(file: Handle, attributes: *const c_void, protect: u32, size_high: u32, size_low: u32, name: *const u16) -> Handle;
    pub fn OpenFileMappingW(access: u32, inherit: i32, name: *const u16) -> Handle;
    pub fn MapViewOfFile(mapping: Handle, access: u32, offset_high: u32, offset_low: u32, bytes: usize) -> *mut c_void;
    pub fn UnmapViewOfFile(address: *const c_void) -> i32;
    pub fn CreateEventW(attributes: *const c_void, manual_reset: i32, initial_state: i32, name: *const u16) -> Handle;
    pub fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> Handle;
    pub fn SetEvent(event: Handle) -> i32;
    pub fn WaitForSingleObject(handle: Handle, milliseconds: u32) -> u32;
    pub fn CloseHandle(handle: Handle) -> i32;
}

/// `s` as a null-terminated UTF-16 string.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
