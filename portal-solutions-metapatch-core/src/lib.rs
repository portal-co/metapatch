#![no_std]

use alloc::vec::Vec;
use waffle::{Block, FunctionBody, Memory, MemoryArg, Module, Operator, Type, Value};
extern crate alloc;
pub mod snapshot;
pub mod wasimap;
pub mod trapcard;
// pub mod paging;