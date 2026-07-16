use crate::error::{Error, Result};
use std::ffi::{CStr, c_char};
use std::ptr::NonNull;
use tonlibjson_sys::tonemulator as ffi;

struct TvmAllocation(NonNull<c_char>);

impl TvmAllocation {
    unsafe fn from_raw(pointer: NonNull<c_char>) -> Self {
        Self(pointer)
    }

    const fn as_ptr(&self) -> *const c_char {
        self.0.as_ptr()
    }
}

impl Drop for TvmAllocation {
    fn drop(&mut self) {
        unsafe { ffi::string_destroy(self.as_ptr()) }
    }
}

pub(crate) fn non_null(pointer: *const c_char) -> Result<NonNull<c_char>> {
    NonNull::new(pointer.cast_mut()).ok_or(Error::Ffi)
}

pub struct TvmString {
    allocation: TvmAllocation,
    len: usize,
}

impl TvmString {
    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn as_str(&self) -> &str {
        unsafe {
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                self.allocation.as_ptr().cast(),
                self.len,
            ))
        }
    }

    pub(crate) unsafe fn from_raw(pointer: NonNull<c_char>) -> Result<Self> {
        let allocation = unsafe { TvmAllocation::from_raw(pointer) };
        let len = unsafe { CStr::from_ptr(pointer.as_ptr()) }.to_bytes().len();
        std::str::from_utf8(unsafe { std::slice::from_raw_parts(pointer.as_ptr().cast(), len) })?;

        Ok(Self { allocation, len })
    }
}

impl AsRef<str> for TvmString {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Debug for TvmString {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_ref().fmt(formatter)
    }
}

pub struct TvmBuffer {
    allocation: TvmAllocation,
    len: usize,
}

impl TvmBuffer {
    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn as_str(&self) -> std::result::Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(self.as_ref())
    }

    pub(crate) unsafe fn from_raw(pointer: NonNull<c_char>) -> Self {
        let allocation = unsafe { TvmAllocation::from_raw(pointer) };
        let len = unsafe {
            u32::from_ne_bytes(std::ptr::read_unaligned(pointer.as_ptr().cast::<[u8; 4]>()))
                as usize
        };

        Self { allocation, len }
    }
}

impl AsRef<[u8]> for TvmBuffer {
    fn as_ref(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.allocation.as_ptr().add(4).cast(), self.len) }
    }
}

impl std::fmt::Debug for TvmBuffer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("TvmBuffer")
            .field(&self.as_ref())
            .finish()
    }
}
