use std::ffi::{c_char, c_int, c_void};

#[link(name = "emulator")]
unsafe extern "C" {
    pub fn transaction_emulator_create(
        config_params_boc: *const c_char,
        vm_log_verbosity: c_int,
    ) -> *mut c_void;

    pub fn transaction_emulator_emulate_transaction(
        p: *mut c_void,
        shard_account_boc: *const c_char,
        message_boc: *const c_char,
    ) -> *const c_char;

    pub fn transaction_emulator_set_unixtime(p: *mut c_void, unixtime: u32) -> bool;

    pub fn transaction_emulator_set_lt(p: *mut c_void, lt: u64) -> bool;

    pub fn transaction_emulator_set_rand_seed(p: *mut c_void, seed: *const c_char) -> bool;

    pub fn transaction_emulator_set_ignore_chksig(p: *mut c_void, ignore_chksig: bool) -> bool;

    pub fn transaction_emulator_set_config(p: *mut c_void, config_boc: *const c_char) -> bool;

    pub fn transaction_emulator_set_libs(p: *mut c_void, libs_boc: *const c_char) -> bool;

    pub fn transaction_emulator_destroy(p: *mut c_void);

    pub fn emulator_set_verbosity_level(level: i32) -> bool;

    pub fn tvm_emulator_create(
        code_boc: *const c_char,
        data_boc: *const c_char,
        vm_log_verbosity: c_int,
    ) -> *mut c_void;

    pub fn tvm_emulator_set_libraries(p: *mut c_void, libs_boc: *const c_char) -> bool;

    pub fn tvm_emulator_set_gas_limit(p: *mut c_void, gas_limit: i64) -> bool;

    pub fn tvm_emulator_set_c7(
        p: *mut c_void,
        address: *const c_char,
        unixtime: u32,
        balance: u64,
        rand_seed_hex: *const c_char,
        config: *const c_char,
    ) -> bool;

    pub fn tvm_emulator_run_get_method(
        p: *mut c_void,
        method_id: c_int,
        stack_boc: *const c_char,
    ) -> *const c_char;

    pub fn tvm_emulator_send_external_message(
        p: *mut c_void,
        message_body_boc: *const c_char,
    ) -> *const c_char;

    pub fn tvm_emulator_send_internal_message(
        p: *mut c_void,
        message_body_boc: *const c_char,
        amount: u64,
    ) -> *const c_char;

    pub fn tvm_emulator_destroy(p: *mut c_void);

    pub fn tvm_emulator_emulate_run_method(
        len: u32,
        params_boc: *const c_char,
        gas_limit: i64,
    ) -> *const c_char;

    pub fn string_destroy(string: *const c_char);
}
