#[link(wasm_import_module = "https://repository.timot.se/test1")]
extern {
    #[link_name = "return_double_arg"]
    fn test1_return_double_arg(arg: i32) -> i32;
}

#[no_mangle]
pub extern fn return_arg(arg: i32) -> i32 {
    unsafe {
        test1_return_double_arg(arg)
    }
}
