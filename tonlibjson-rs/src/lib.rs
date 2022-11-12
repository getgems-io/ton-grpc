use std::{ffi::{c_void, CStr, CString}, time::Duration};
use libc::{c_char, c_double, c_int};
use anyhow::{anyhow, Result};

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use crate::Client;

    #[test]
    fn receive_timeout() {
        let client = Client::new();
        let response = client.receive(Duration::from_micros(10));

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().to_string(), "timeout reached")
    }

    #[test]
    fn call_send() {
        let client = Client::new();
        let response = client.send("query");

        assert!(response.is_ok())
    }

    #[test]
    fn call_execute() {
        let client = Client::new();
        let response = client.execute("query");

        assert!(response.is_err())
    }

    #[test]
    fn set_verbosity_level() {
        Client::set_verbosity_level(0)
    }
}

#[link(name = "tonlib")]
extern {
    fn tonlib_client_json_create() -> *mut c_void;

    fn tonlib_client_json_destroy(p: *mut c_void);

    fn tonlib_client_json_send(p: *mut c_void, request: *const c_char);

    fn tonlib_client_json_execute(p: *mut c_void, request: *const c_char) -> *const c_char;

    fn tonlib_client_json_receive(p: *mut c_void, timeout: c_double) -> *const c_char;

    fn tonlib_client_set_verbosity_level(level: c_int);
}

#[derive(Debug)]
pub struct Client {
    pointer: *mut c_void,
}

impl Client {
    pub fn new() -> Self {
        Self {
            pointer: unsafe { tonlib_client_json_create() }
        }
    }

    pub fn set_verbosity_level(level: i32) {
        unsafe { tonlib_client_set_verbosity_level(level) }
    }

    pub fn send(&self, request: &str) -> Result<()> {
        let req = CString::new(request)?;

        unsafe { tonlib_client_json_send(self.pointer, req.as_ptr()); }

        Ok(())
    }

    pub fn receive(&self, timeout: Duration) -> Result<&str> {
        let response = unsafe {
            let ptr = tonlib_client_json_receive(self.pointer, timeout.as_secs_f64());
            if ptr.is_null() {
                return Err(anyhow!("timeout reached"));
            }

            CStr::from_ptr(ptr)
        };

        let str = response.to_str()?;

        Ok(str)
    }

    pub fn execute(&self, request: &str) -> Result<&str> {
        let req = CString::new(request)?;

        let response = unsafe {
            let ptr = tonlib_client_json_execute(self.pointer, req.into_raw());
            if ptr.is_null() {
                return Err(anyhow!("timeout reached"));
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

impl Drop for Client {
    fn drop(&mut self) {
        unsafe { tonlib_client_json_destroy(self.pointer) }
    }
}

unsafe impl Send for Client {}

unsafe impl Sync for Client {}
