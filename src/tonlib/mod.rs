use std::ffi::{c_void, CStr, CString};
use std::time::Duration;
use libc::c_char;
use serde_json::json;

use libc::c_double;

#[link(name = "tonlib")]
extern {
    fn tonlib_client_json_create() -> *mut c_void;

    fn tonlib_client_json_destroy(p: *mut c_void) -> ();

    fn tonlib_client_json_send(p: *mut c_void, request: *const c_char) -> ();

    fn tonlib_client_json_execute(p: *mut c_void, request: *const c_char) -> *const c_char;

    fn tonlib_client_json_receive(p: *mut c_void, timeout: c_double) -> *const c_char;
}

pub struct Client {
    pointer: *mut c_void,
}

impl Client {
    pub fn new() -> Self {
        let p = unsafe { tonlib_client_json_create() };

        let client = Client {
            pointer: p
        };

        client.initialize();

        return client;
    }

    fn initialize(&self) {
        let request = json!({
            "@type": "init",
            "options": {
                "@type": "options",
                "config": {
                    "@type": "config",
                    "config": self.config(),
                    "use_callbacks_for_network": false,
                    "blockchain_name": "",
                    "ignore_cache": false
                },
                "keystore_type": {
                    "@type": "keyStoreTypeDirectory",
                    "directory": "./ton_keystore/"
                }
            }
        });

        let _ = self.send(request.to_string().as_str());
    }

    pub fn send(&self, request: &str) -> () {
        let req = CString::new(request).unwrap();

        unsafe {
            tonlib_client_json_send(self.pointer, req.as_ptr());
        }
    }

    pub fn receive(&self, timeout: Duration) -> Option<&str> {
        let response = unsafe {
            let t = c_double::from(timeout.as_secs_f64());
            let ptr = tonlib_client_json_receive(self.pointer, t);
            if ptr.is_null() {
                return None;
            }

            CStr::from_ptr(ptr)
        };

        return response.to_str().ok();
    }

    pub fn execute(&self, request: &str) -> Option<&str> {
        let req = CString::new(request);
        if let Err(e) = req {
            println!("{}", e);

            return None;
        }

        let response = unsafe {
            let ptr = tonlib_client_json_execute(self.pointer, req.unwrap().into_raw());
            if ptr.is_null() {
                return None;
            }

            CStr::from_ptr(ptr)
        };

        return response.to_str().ok();
    }

    fn config(&self) -> &'static str {
        let config = include_str!("liteserver_config.json");

        return config;
    }
}

unsafe impl Send for Client {}
unsafe impl Sync for Client {}

impl Drop for Client {
    fn drop(&mut self) {
        println!("destroy");
        unsafe { tonlib_client_json_destroy(self.pointer) }
    }
}
