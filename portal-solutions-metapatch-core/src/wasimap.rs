use crate::*;
use alloc::{collections::btree_set::BTreeSet, format};
use anyhow::Context;
use waffle::{
    ExportKind, Func, Module,
    copying::fcopy::{DontObf, Obfuscate, obf_mod},
    entity::EntityRef,
    util::new_sig,
};

pub fn wasimap(module: &mut Module<'_>, mut wasi_memory: Memory) -> anyhow::Result<()> {
    waffle::passes::mapping::lower(module, |m| m != wasi_memory)?;
    struct Runtime {
        resolver: Func,
        size: Func,
        grow: Func,
    }
    impl Runtime {
        fn of(m: &Module) -> anyhow::Result<Self> {
            Ok(Self {
                resolver: m
                    .exports
                    .iter()
                    .find_map(|a| {
                        if a.name == "wasimap_resolve" {
                            match &a.kind {
                                ExportKind::Func(f) => Some(*f),
                                _ => None,
                            }
                        } else {
                            None
                        }
                    })
                    .context("in getting the resolver")?,
                size: m
                    .exports
                    .iter()
                    .find_map(|a| {
                        if a.name == "wasimap_size" {
                            match &a.kind {
                                ExportKind::Func(f) => Some(*f),
                                _ => None,
                            }
                        } else {
                            None
                        }
                    })
                    .context("in getting the size getter")?,
                grow: m
                    .exports
                    .iter()
                    .find_map(|a| {
                        if a.name == "wasimap_grow" {
                            match &a.kind {
                                ExportKind::Func(f) => Some(*f),
                                _ => None,
                            }
                        } else {
                            None
                        }
                    })
                    .context("in getting the grower")?,
            })
        }
    }
    let rt = Runtime::of(module)?;
    let sig = new_sig(
        module,
        waffle::SignatureData::Func {
            params: Default::default(),
            returns: Default::default(),
            shared: false,
        },
    );
    let mut body = FunctionBody::new(module, sig);

    for m in module.memories.iter() {
        if m == wasi_memory {
            continue;
        }
        let [memid, start, len] = [
            m.index() as u32,
            0,
            (module.memories[m].initial_pages << (module.memories[m].page_size_log2.unwrap_or(16)))
                as u32,
        ]
        .map(|a| {
            body.add_op(
                body.entry,
                Operator::I32Const { value: a },
                &[],
                &[Type::I32],
            )
        });
        body.add_op(
            body.entry,
            Operator::Call {
                function_index: rt.grow,
            },
            &[memid, len],
            &[],
        );
        let i = body.add_op(
            body.entry,
            Operator::Call {
                function_index: rt.resolver,
            },
            &[memid, start, len],
            &[Type::I32],
        );
        body.add_op(
            body.entry,
            Operator::MemoryCopy {
                dst_mem: wasi_memory,
                src_mem: m,
            },
            &[i, start, len],
            &[],
        );
    }
    if let Some(f) = module.start_func.take() {
        body.set_terminator(
            body.entry,
            waffle::Terminator::ReturnCall {
                func: f,
                args: Vec::default(),
            },
        );
    } else {
        body.set_terminator(
            body.entry,
            waffle::Terminator::Return {
                values: Default::default(),
            },
        );
    }
    obf_mod(module, &mut Wasimap { wasi_memory, rt })?;
    module.start_func = Some(module.funcs.push(waffle::FuncDecl::Body(
        sig,
        format!("<start>"),
        body,
    )));
    struct Wasimap {
        wasi_memory: Memory,
        rt: Runtime,
        // body: &'a mut FunctionBody,
    }
    impl Obfuscate for Wasimap {
        fn obf(
            &mut self,
            o: Operator,
            f: &mut FunctionBody,
            b: Block,
            args: &[Value],
            types: &[Type],
            module: &mut Module,
        ) -> anyhow::Result<(Value, Block)> {
            match o {
                Operator::I32Load8U { mut memory } if memory.memory != self.wasi_memory => {
                    let [memid, len] = [memory.memory.index() as u32, 1]
                        .map(|a| f.add_op(b, Operator::I32Const { value: a }, &[], &[Type::I32]));
                    let start = args[0];
                    let i = f.add_op(
                        b,
                        Operator::Call {
                            function_index: self.rt.resolver,
                        },
                        &[memid, start, len],
                        &[Type::I32],
                    );
                    memory.memory = self.wasi_memory;
                    return DontObf {}.obf(
                        Operator::I32Load8U { memory },
                        f,
                        b,
                        &[i],
                        types,
                        module,
                    );
                }
                Operator::I32Store8 { mut memory } if memory.memory != self.wasi_memory => {
                    let [memid, len] = [memory.memory.index() as u32, 1]
                        .map(|a| f.add_op(b, Operator::I32Const { value: a }, &[], &[Type::I32]));
                    let start = args[0];
                    let i = f.add_op(
                        b,
                        Operator::Call {
                            function_index: self.rt.resolver,
                        },
                        &[memid, start, len],
                        &[Type::I32],
                    );
                    memory.memory = self.wasi_memory;
                    return DontObf {}.obf(
                        Operator::I32Store8 { memory },
                        f,
                        b,
                        &[i, args[1]],
                        types,
                        module,
                    );
                }
                Operator::MemorySize { mem } if mem != self.wasi_memory => {
                    let [memid, len] = [
                        mem.index() as u32,
                        module.memories[mem].page_size_log2.unwrap_or(16),
                    ]
                    .map(|a| f.add_op(b, Operator::I32Const { value: a }, &[], &[Type::I32]));
                    // let arg = f.add_op(b,Operator::I32Shl,&[args[0],len],&[Type::I32]);
                    let val = f.add_op(
                        b,
                        Operator::Call {
                            function_index: self.rt.size,
                        },
                        &[memid],
                        &[Type::I32],
                    );
                    DontObf {}.obf(Operator::I32ShrU, f, b, &[val, len], types, module)
                }
                Operator::MemoryGrow { mem } if mem != self.wasi_memory => {
                    let [memid, len] = [
                        mem.index() as u32,
                        module.memories[mem].page_size_log2.unwrap_or(16),
                    ]
                    .map(|a| f.add_op(b, Operator::I32Const { value: a }, &[], &[Type::I32]));
                    let arg = f.add_op(b, Operator::I32Shl, &[args[0], len], &[Type::I32]);
                    let val = f.add_op(
                        b,
                        Operator::Call {
                            function_index: self.rt.grow,
                        },
                        &[memid, arg],
                        &[Type::I32],
                    );
                    DontObf {}.obf(Operator::I32ShrU, f, b, &[val, len], types, module)
                }
                o => DontObf {}.obf(o, f, b, args, types, module),
            }
        }
    }
    Ok(())
}
