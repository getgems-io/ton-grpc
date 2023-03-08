use std::ffi::{c_void, CStr, CString};
use libc::{c_int, c_char, c_uint, c_ulong, c_long};
use anyhow::{anyhow, Result};

#[link(name = "emulator")]
extern {
    fn tvm_emulator_create(code_boc: *const c_char, data_boc: *const c_char, vm_log_verbosity: c_int) -> *mut c_void;

    fn tvm_emulator_set_libraries(p: *mut c_void, libs_boc: *const c_char) -> bool;

    fn tvm_emulator_set_gas_limit(p: *mut c_void, gas_limit: c_long) -> bool;

    fn tvm_emulator_set_c7(p: *mut c_void, address: *const c_char, unixtime: c_uint, balance: c_ulong, rand_seed_hex: *const c_char, config: *const c_char) -> bool;

    fn tvm_emulator_run_get_method(p: *mut c_void, method_id: c_int, stack_boc: *const c_char) -> *const c_char;

    fn tvm_emulator_send_external_message(p: *mut c_void, message_body_boc: *const c_char) -> *const c_char;

    fn tvm_emulator_send_internal_message(p: *mut c_void, message_body_boc: *const c_char, amount: c_ulong) -> *const c_char;

    fn tvm_emulator_destroy(p: *mut c_void);
}

#[derive(Debug)]
pub struct TvmEmulator {
    pointer: *mut c_void,
}

impl TvmEmulator {
    pub fn new(code: &str, data: &str, vm_log_verbosity: i32) -> Result<Self> {
        let code = CString::new(code)?;
        let data = CString::new(data)?;

        Ok(Self {
            pointer: unsafe { tvm_emulator_create(code.as_ptr(), data.as_ptr(), vm_log_verbosity) }
        })
    }

    // TODO result
    pub fn set_libraries(&self, libs_boc: &str) -> Result<bool> {
        let req = CString::new(libs_boc)?;

        Ok(unsafe { tvm_emulator_set_libraries(self.pointer, req.as_ptr()) })
    }

    pub fn set_gas_limit(&self, gas_limit: i64) -> bool {
        unsafe { tvm_emulator_set_gas_limit(self.pointer, gas_limit) }
    }

    pub fn set_c7(&self, address: &str, unixtime: u32, balance: u64, rand_seed_hex: &str, config: &str) -> Result<bool> {
        let address = CString::new(address)?;
        let rand_seed_hex = CString::new(rand_seed_hex)?;
        let config = CString::new(config)?;

        Ok(unsafe { tvm_emulator_set_c7(
            self.pointer,
            address.as_ptr(),
            unixtime,
            balance,
            rand_seed_hex.as_ptr(),
            config.as_ptr()
        )})
    }

    pub fn run_get_method(&self, method_id: i32, stack_boc: &str) -> Result<&str> {
        let stack_boc = CString::new(stack_boc)?;

        let result = unsafe {
            let ptr = tvm_emulator_run_get_method(self.pointer, method_id, stack_boc.as_ptr());
            if ptr.is_null() {
                return Err(anyhow!("pointer is null"));
            }

            CStr::from_ptr(ptr)
        };

        Ok(result.to_str()?)
    }

    pub fn send_external_message(&self, message_body_boc: &str) -> Result<&str> {
        let message_body_boc = CString::new(message_body_boc)?;

        let result = unsafe {
            let ptr = tvm_emulator_send_external_message(self.pointer, message_body_boc.as_ptr());
            if ptr.is_null() {
                return Err(anyhow!("pointer is null"));
            }

            CStr::from_ptr(ptr)
        };

        Ok(result.to_str()?)
    }

    pub fn send_internal_message(&self, message_body_boc: &str, amount: u64) -> Result<&str> {
        let message_body_boc = CString::new(message_body_boc)?;

        let result = unsafe {
            let ptr = tvm_emulator_send_internal_message(self.pointer, message_body_boc.as_ptr(), amount);
            if ptr.is_null() {
                return Err(anyhow!("pointer is null"));
            }

            CStr::from_ptr(ptr)
        };

        Ok(result.to_str()?)
    }
}

impl Drop for TvmEmulator {
    fn drop(&mut self) {
        unsafe {
            tvm_emulator_destroy(self.pointer)
        }
    }
}

#[cfg(test)]
pub mod tests {
    use crate::tonemulator::{TvmEmulator, emulator_set_verbosity_level};

    #[test]
    fn emulator_set_verbosity_level_test() {
        let x = unsafe { emulator_set_verbosity_level(1) };

        assert!(x)
    }

    #[test]
    fn tvm_emulator_create_test() {
        let code = "te6cckEBAQEAcQAA3v8AIN0gggFMl7ohggEznLqxn3Gw7UTQ0x/THzHXC//jBOCk8mCDCNcYINMf0x/TH/gjE7vyY+1E0NMf0x/T/9FRMrryoVFEuvKiBPkBVBBV+RDyo/gAkyDXSpbTB9QC+wDo0QGkyMsfyx/L/8ntVBC9ba0=";
        let data = "te6cckEBAQEAKgAAUAAAAAspqaMXeFMWBTkvznPWzYwz6MYIKICIXmLZbe9Dp1kz2XjSSeprfXb5";
        let p = TvmEmulator::new(code, data, 1);

        assert!(p.is_ok());

        let emulator = p.unwrap();

        // TODO
        assert!(emulator.set_libraries("te6cckEBAQE").is_ok());
        assert!(emulator.set_gas_limit(1000));
    }

    #[test]
    fn tvm_run_get_method_test() {
        let code = "te6cckECDQEAAdAAART/APSkE/S88sgLAQIBYgIDAgLOBAUACaEfn+AFAgEgBgcCASALDALXDIhxwCSXwPg0NMDAXGwkl8D4PpA+kAx+gAxcdch+gAx+gAw8AIEs44UMGwiNFIyxwXy4ZUB+kDUMBAj8APgBtMf0z+CEF/MPRRSMLqOhzIQN14yQBPgMDQ0NTWCEC/LJqISuuMCXwSED/LwgCAkAET6RDBwuvLhTYAH2UTXHBfLhkfpAIfAB+kDSADH6AIIK+vCAG6EhlFMVoKHeItcLAcMAIJIGoZE24iDC//LhkiGOPoIQBRONkchQCc8WUAvPFnEkSRRURqBwgBDIywVQB88WUAX6AhXLahLLH8s/Im6zlFjPFwGRMuIByQH7ABBHlBAqN1viCgBycIIQi3cXNQXIy/9QBM8WECSAQHCAEMjLBVAHzxZQBfoCFctqEssfyz8ibrOUWM8XAZEy4gHJAfsAAIICjjUm8AGCENUydtsQN0QAbXFwgBDIywVQB88WUAX6AhXLahLLH8s/Im6zlFjPFwGRMuIByQH7AJMwMjTiVQLwAwA7O1E0NM/+kAg10nCAJp/AfpA1DAQJBAj4DBwWW1tgAB0A8jLP1jPFgHPFszJ7VSC/dQQb";
        let data = "te6cckEBAgEAXQABlQAAAAAAAADWgA4AJo/0FYx9xbxf+LSSVFEobkVuncS4aWpEFnq/ph29cAJpq11GV5cd0wJzc41CxmSVwXfaPIKImEfI3dgQrE9cygEAGjIxNC9tZXRhLmpzb24K7pLP";

        let emulator = TvmEmulator::new(code, data, 1).unwrap();

        // TODO
        let result = emulator.run_get_method(0, "");

        println!("{:?}", result);
        assert!(result.is_ok());
    }
}
