use std::ffi::{c_void, CString};
use libc::{c_int, c_char};
use anyhow::Result;

#[link(name = "emulator")]
extern {
    fn emulator_set_verbosity_level(verbosity_level: c_int) -> bool;

    fn tvm_emulator_create(code_boc: *const c_char, data_boc: *const c_char, vm_log_verbosity: c_int) -> *mut c_void;

    // fn tvm_emulator_set_libraries() -> ;
}

#[cfg(test)]
pub mod tests {
    use crate::tonemulator::{Emulator, emulator_set_verbosity_level, tvm_emulator_create};

    #[test]
    fn emulator_set_verbosity_level_test() {
        let x = unsafe { emulator_set_verbosity_level(1) };

        assert!(x)
    }

    #[test]
    fn tvm_emulator_create_test() {
        let code = "te6cckEBAQEAcQAA3v8AIN0gggFMl7ohggEznLqxn3Gw7UTQ0x/THzHXC//jBOCk8mCDCNcYINMf0x/TH/gjE7vyY+1E0NMf0x/T/9FRMrryoVFEuvKiBPkBVBBV+RDyo/gAkyDXSpbTB9QC+wDo0QGkyMsfyx/L/8ntVBC9ba0=";
        let data = "te6cckEBAQEAKgAAUAAAAAspqaMXeFMWBTkvznPWzYwz6MYIKICIXmLZbe9Dp1kz2XjSSeprfXb5";
        let p = Emulator::new(code, data, 1);

        println!("{:?}", p);

        assert!(p.is_ok())
    }
}

#[derive(Debug)]
pub struct Emulator {
    pointer: *mut c_void,
}

impl Emulator {
    pub fn new(code: &str, data: &str, vm_log_verbosity: i32) -> Result<Self> {
        let code = CString::new(code)?;
        let data = CString::new(data)?;

        Ok(Self {
            pointer: unsafe { tvm_emulator_create(code.as_ptr(), data.as_ptr(), vm_log_verbosity) }
        })
    }
}
