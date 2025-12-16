#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(asm_const)]

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    exokernel::panic_handler(info)
}
