// src/console/mod.rs
//! 控制台输出

use core::fmt::{self, Write};

struct Console;

impl Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            crate::arch::write_serial(byte);
        }
        Ok(())
    }
}

pub fn init() {
    crate::arch::X86_64::early_init();
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    let mut console = Console;
    console.write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::console::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ({
        $crate::console::_print(format_args_nl!($($arg)*));
    })
}