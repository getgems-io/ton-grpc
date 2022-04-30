use std::collections::HashMap;
use std::ffi::{c_void, CStr, CString};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use libc::c_char;
use serde_json::{json, Value};

use libc::c_double;
use tokio::sync::mpsc::{Sender, Receiver, UnboundedSender};
use tokio::sync::oneshot::error::RecvError;
use uuid::Uuid;

#[link(name = "tonlib")]
extern {
    fn tonlib_client_json_create() -> *mut c_void;

    fn tonlib_client_json_destroy(p: *mut c_void) -> ();

    fn tonlib_client_json_send(p: *mut c_void, request: *const c_char) -> ();

    fn tonlib_client_json_execute(p: *mut c_void, request: *const c_char) -> *const c_char;

    fn tonlib_client_json_receive(p: *mut c_void, timeout: c_double) -> *const c_char;
}

pub struct Client {
    pointer: *mut c_void
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
        let config = json!({
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

        let _ = self.send(config.to_string().as_str());

        let verbosity = json!({
            "@type": "setLogVerbosityLevel",
            "new_verbosity_level": 0
        });

        let _ = self.send(verbosity.to_string().as_str());
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
            // println!("{}", e);

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
        unsafe { tonlib_client_json_destroy(self.pointer) }
    }
}


pub struct AsyncClient {
    client: Arc<Client>,
    receive_thread: JoinHandle<()>,
    responses: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<Value>>>>
}

impl AsyncClient {
    pub fn new() -> Self{
        let client = Arc::new(Client::new());
        let client_recv = client.clone();

        let responses: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<Value>>>> = Arc::new(Mutex::new(HashMap::new()));
        let responses_rcv = Arc::clone(&responses);

        let receive_thread = thread::spawn(move || {
            let timeout = Duration::from_secs(20);
            loop {
                let packet = client_recv.receive(timeout);
                let json: Option<Value> = packet.and_then(|x| serde_json::from_str::<Value>(x).ok());
                match json {
                    Some(v) => {
                        let extra = v.get("@extra");
                        // println!("{:#?}", extra);

                        if let Some(Value::String(id)) = extra {
                            let mut resps = responses_rcv.lock().unwrap();
                            let s = resps.remove::<String>(id);
                            drop(resps);
                            if let Some(s) = s {
                                let _ = s.send(v);
                            }
                        }
                    },
                    None => {}
                }
            }
        });

        return AsyncClient{
            client,
            receive_thread,
            responses
        }
    }

    pub async fn send(&self, request: serde_json::Value) -> () {
        self.client.send(&request.to_string());
    }

    pub async fn execute(&self, mut request: serde_json::Value) -> Result<Value, RecvError> {
        let id = Uuid::new_v4().to_string();
        request["@extra"] = json!(id);
        let (tx, rx) = tokio::sync::oneshot::channel::<Value>();
        self.responses.lock().unwrap().insert(id, tx);

        self.client.send(&request.to_string());

        return rx.await
    }
}
