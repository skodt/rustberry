#![no_std]
#![feature(asm, exact_chunks)]
#![allow(dead_code)]

#[macro_use] extern crate bitflags;

#[macro_use] pub mod coproc_reg;
#[macro_use] pub mod mmio;
mod bcm2708;
mod quad_a7;

pub use bcm2708::{uart, gpio, system_timer, video_core, random, emmc};
pub use quad_a7::{interrupts, get_core_id, mailbox, core_timer};
