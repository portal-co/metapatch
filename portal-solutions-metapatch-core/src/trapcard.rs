use core::cell::OnceCell;

use alloc::collections::btree_set::BTreeSet;
use rand::{Rng, RngCore, seq::IndexedRandom};
use waffle::{BlockTarget, ExportKind, Func, Global, GlobalData, entity::PerEntity};

use crate::*;
pub struct Manifest(pub PerEntity<Func,u64>,pub Global);
pub fn all(module: &mut Module, rng: &mut (dyn RngCore + '_)) -> Manifest {
    let f: BTreeSet<Func> = module.funcs.iter().collect();
    let mut never_main = f.clone();
    for x in module.exports.iter() {
        if let ExportKind::Func(f) = &x.kind {
            never_main.remove(f);
        }
    }
    return core(module, rng, &f, &never_main);
}
pub fn core(
    module: &mut Module,
    rng: &mut (dyn RngCore + '_),
    f: &BTreeSet<Func>,
    never_main: &BTreeSet<Func>,
) -> Manifest {
    let mut p: PerEntity<Func, u64> = PerEntity::default();
    let gv = rng.random();
    let g = module.globals.push(GlobalData {
        ty: Type::I64,
        value: Some(gv),
        mutable: true,
    });
    let h = OnceCell::new();
    for f2 in f.iter().cloned() {
        waffle::hooking::with_swizz(module, f2, &mut (), |(module, f, b, _)| {
            let r: u64 = rng.random();
            p[f2] = r;
            let v = f.add_op(
                f.entry,
                Operator::GlobalGet { global_index: g },
                &[],
                &[Type::I64],
            );
            let w = f.add_op(f.entry, Operator::I64Const { value: r }, &[], &[Type::I64]);
            let v = f.add_op(f.entry, Operator::I64Xor, &[v, w], &[Type::I64]);
            let one = f.add_op(f.entry, Operator::I32Const { value: 1 }, &[], &[Type::I32]);
            let v = f.add_op(f.entry, Operator::I64Rotl, &[v, one], &[Type::I64]);
            f.add_op(f.entry, Operator::GlobalSet { global_index: g }, &[v], &[]);
            let args: Vec<Value> = f.blocks[f.entry].params.iter().map(|a| a.1).collect();
            if never_main.len() == 0 {
                f.set_terminator(f.entry, waffle::Terminator::ReturnCall { func: b, args });
            } else {
                let h = *h.get_or_init(|| {
                    module.globals.push(GlobalData {
                        ty: Type::I32,
                        value: Some(0),
                        mutable: true,
                    })
                });
                let c = (gv ^ r).rotate_left(1);
                let c = f.add_op(f.entry, Operator::I64Const { value: c }, &[], &[Type::I64]);
                let c = f.add_op(f.entry, Operator::I64Ne, &[c, v], &[Type::I32]);
                let hv = f.add_op(
                    f.entry,
                    Operator::GlobalGet { global_index: h },
                    &[],
                    &[Type::I32],
                );
                let c = f.add_op(f.entry, Operator::I32Or, &[c, hv], &[Type::I32]);
                let y = f.add_block();
                f.add_op(y, Operator::GlobalSet { global_index: h }, &[c], &[]);
                f.set_terminator(
                    y,
                    waffle::Terminator::ReturnCall {
                        func: b,
                        args: args.clone(),
                    },
                );
                let n = f.add_block();
                f.set_terminator(
                    n,
                    waffle::Terminator::ReturnCall {
                        func: if never_main.contains(&f2) {
                            let r = module
                                .funcs
                                .iter()
                                .filter(|a| *a != b)
                                .collect::<BTreeSet<_>>()
                                .into_iter()
                                .filter(|r| module.funcs[*r].sig() == module.funcs[b].sig())
                                .collect::<Vec<_>>();
                            let r = r.choose(rng).cloned().unwrap();
                            r
                        } else {
                            b
                        },
                        args,
                    },
                );
                f.set_terminator(
                    f.entry,
                    waffle::Terminator::CondBr {
                        cond: c,
                        if_true: BlockTarget {
                            block: y,
                            args: Default::default(),
                        },
                        if_false: BlockTarget {
                            block: n,
                            args: Default::default(),
                        },
                    },
                );
            }
        });
    }
    Manifest(p, g)
}
