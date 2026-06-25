use anyhow::{Result, anyhow};
use libc::{c_char, c_double, c_int};
use std::{
    ffi::{CStr, CString, c_void},
    ptr::NonNull,
    time::Duration,
};

#[cfg(test)]
mod tests {
    use crate::tonlibjson::Client;
    use std::assert_matches;
    use std::time::Duration;

    #[test]
    fn receive_timeout() {
        let client = Client::new();
        let response = client.receive(Duration::from_micros(10));

        assert_matches!(response, Ok(None));
    }

    #[test]
    fn call_send_invalid_query() {
        let client = Client::new();
        let response = client.send("query");

        assert_matches!(response, Ok(()))
    }

    #[test]
    fn call_execute_invalid_query() {
        let client = Client::new();

        let response = client.execute("query");

        assert_eq!(response.unwrap_err().to_string(), "null received")
    }

    #[test]
    fn call_execute() {
        let client = Client::new();

        let response = client.execute("{\"@type\": \"blocks.getMasterchainInfo\"}");

        assert_eq!(
            response.unwrap(),
            "{\"@type\":\"error\",\"code\":400,\"message\":\"Function can't be executed synchronously\"}"
        )
    }

    #[test]
    fn set_verbosity_level() {
        Client::set_verbosity_level(0)
    }

    #[test]
    fn clear_thread_locals() {
        Client::clear_thread_locals()
    }
}

#[link(name = "tonlib")]
unsafe extern "C" {
    fn tonlib_client_json_create() -> *mut c_void;

    fn tonlib_client_json_destroy(p: *mut c_void);

    fn tonlib_client_json_send(p: *mut c_void, request: *const c_char);

    fn tonlib_client_json_execute(p: *mut c_void, request: *const c_char) -> *const c_char;

    fn tonlib_client_json_receive(p: *mut c_void, timeout: c_double) -> *const c_char;

    fn tonlib_client_set_verbosity_level(level: c_int);
}

unsafe extern "C" {
    fn td_clear_thread_locals();
}

#[derive(Debug)]
pub struct Client {
    pointer: NonNull<c_void>,
}

impl Client {
    pub fn new() -> Self {
        let pointer = unsafe { NonNull::new_unchecked(tonlib_client_json_create()) };

        Self { pointer }
    }

    pub fn set_verbosity_level(level: i32) {
        unsafe { tonlib_client_set_verbosity_level(level) }
    }

    pub fn clear_thread_locals() {
        unsafe { td_clear_thread_locals() }
    }

    pub fn send(&self, request: &str) -> Result<()> {
        tracing::trace!(request = request);

        let req = CString::new(request)?;

        unsafe {
            tonlib_client_json_send(self.pointer.as_ptr(), req.as_ptr());
        }

        Ok(())
    }

    pub fn receive(&self, timeout: Duration) -> Result<Option<&str>> {
        let response = unsafe {
            let ptr = tonlib_client_json_receive(self.pointer.as_ptr(), timeout.as_secs_f64());
            if ptr.is_null() {
                return Ok(None);
            }

            CStr::from_ptr(ptr)
        };

        Ok(Some(response.to_str()?))
    }

    pub fn execute(&self, request: &str) -> Result<&str> {
        let req = CString::new(request)?;

        let response = unsafe {
            let ptr = tonlib_client_json_execute(self.pointer.as_ptr(), req.into_raw());
            if ptr.is_null() {
                return Err(anyhow!("null received"));
            }

            CStr::from_ptr(ptr)
        };

        let str = response.to_str()?;

        Ok(str)
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

// TODO[akostylev0]: clear_thread_locals
impl Drop for Client {
    fn drop(&mut self) {
        unsafe { tonlib_client_json_destroy(self.pointer.as_ptr()) }
    }
}

// TODO[akostylev0]: thread local
unsafe impl Send for Client {}

unsafe impl Sync for Client {}
