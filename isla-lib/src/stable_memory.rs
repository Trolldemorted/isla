use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use crate::concrete::BV;
use crate::error::ExecError;
use crate::ir;
use crate::ir::Val;
use crate::log;
use crate::memory::{CustomRegion, Address};
use crate::smt::smtlib::{Def, Exp, Ty};
use crate::smt::{Event, SmtResult, Solver, Sym};

#[derive(Clone)]
pub struct StableMemoryRegion<B: BV> {
    data: HashMap<Address, Val<B>>
}

impl<B: BV> StableMemoryRegion<B> {
    pub fn new() -> Self {
        StableMemoryRegion {
            data: HashMap::new()
        }
    }
}

impl<B: 'static + BV> CustomRegion<B> for StableMemoryRegion<B> {
    fn read(
        &mut self,
        read_kind: Val<B>,
        address: Address,
        bytes: u32,
        solver: &mut Solver<B>,
        tag: bool,
    ) -> Result<Val<B>, ExecError> {
        let bit_len = bytes * 8;
        println!("MEMREAD foo = [{:#018X}]", address);
        if let Some(read) = self.data.get(&address) {
            // Direct match!
            match read {
                Val::Bits(b) => {
                    if b.len() < bit_len {
                        // The saved Val is not big enough
                        // TODO: properly constrain the symbol to have the lower bits constrained
                        let value = solver.fresh();
                        solver.add(Def::DeclareConst(value, Ty::BitVec(8 * bytes)));
                        Ok(Val::Symbolic(value))
                    }
                    else if b.len() == bit_len {
                        Ok(Val::Bits(*b))
                    } else {
                        // The saved Val is too big
                        Ok(Val::Bits(b.slice(0, bit_len).unwrap()))
                    }
                },
                Val::Symbolic(s) => {
                    //TODO do it properly: the accessed memory region may partially spread across multiple Vals
                    let value = solver.fresh();
                    solver.add(Def::DeclareConst(value, Ty::BitVec(8 * bytes)));
                    Ok(Val::Symbolic(value))
                },
                _ => Err(ExecError::BadRead)
            }
        } else {
            // Nothing found, but there could be a wider value to the left
            for i in 1..7 {
                if let Some(left) = self.data.get(&(address - i)) {
                    let overlap_bit_width: u32 = 8 * i as u32;
                    match left {
                        Val::Bits(b) => {
                            if b.len() + overlap_bit_width < bit_len {
                                // Value is not big enough
                                unimplemented!()
                            } else {
                                assert!(b.len() >= bit_len + 8);
                                return Ok(Val::Bits(b.slice(overlap_bit_width, bit_len).unwrap()))
                            } 
                        },
                        _ => unimplemented!()
                    }
                }
            }
    
            // Nothing to the left, so let's return a new symbol
            let value = solver.fresh();
            solver.add(Def::DeclareConst(value, Ty::BitVec(bit_len)));
            //region.insert(address, Val::Symbolic(value));
            Ok(Val::Symbolic(value))
        }
    }

    fn write(
        &mut self,
        read_kind: Val<B>,
        address: Address,
        data: Val<B>,
        solver: &mut Solver<B>,
        tag: Option<Val<B>>,
    ) -> Result<Val<B>, ExecError> {
        self.data.insert(address, data);
        // TODO properly overwrite the next cells to the right.
        // Right now a read from a higher address can return a stale value.
        Ok(Val::Unit)
    }

    fn initial_value(&self, address: Address, bytes: u32) -> Option<B> {
        unimplemented!()
    }

    fn clone_dyn(&self) -> Box<dyn Send + Sync + CustomRegion<B>> {
        Box::new(self.clone())
    }
}