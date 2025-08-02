use core::ops::Range;

use crate::*;
#[derive(Clone)]
pub struct Snapshot {
    all: Vec<Value>,
    mem: Memory,
    memory64: bool,
    start: u64,
    zero: Value,
}
impl Snapshot {
    pub fn starting_addr(&self) -> u64 {
        self.start
    }
    pub fn byte_length(&self) -> usize {
        self.all.len() * 8
    }
    pub fn byte_range(&self) -> Range<u64> {
        let start = self.starting_addr();
        start..(start + (self.byte_length() as u64))
    }
    pub fn get_by_range(
        f: &mut FunctionBody,
        module: &Module,
        mem: Memory,
        k: Block,
        range: Range<u64>,
    ) -> Self {
        Self::get(
            f,
            module,
            mem,
            k,
            range.start,
            (((range.end - range.start) + 7) >> 3) as usize,
        )
    }
    pub fn get(
        f: &mut FunctionBody,
        module: &Module,
        mem: Memory,
        k: Block,
        start: u64,
        len: usize,
    ) -> Self {
        let memory64 = module.memories[mem].memory64;
        let zero = f.add_op(
            k,
            if memory64 {
                Operator::I64Const { value: 0 }
            } else {
                Operator::I32Const { value: 0 }
            },
            &[],
            &[if memory64 { Type::I64 } else { Type::I32 }],
        );
        Self {
            all: (0..len)
                .map(|a| {
                    f.add_op(
                        k,
                        Operator::I64Load {
                            memory: MemoryArg {
                                align: 3,
                                offset: (a as u64) * 8 + start,
                                memory: mem,
                            },
                        },
                        &[zero],
                        &[Type::I64],
                    )
                })
                .collect(),
            mem,
            memory64,
            start,
            zero,
        }
    }
    pub fn render(&self, f: &mut FunctionBody, k: Block) {
        for (a, v) in self.all.iter().cloned().enumerate() {
            f.add_op(
                k,
                Operator::I64Store {
                    memory: MemoryArg {
                        align: 3,
                        offset: (a as u64) * 8 + self.start,
                        memory: self.mem,
                    },
                },
                &[self.zero, v],
                &[],
            );
        }
    }
}
