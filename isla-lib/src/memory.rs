// BSD 2-Clause License
//
// Copyright (c) 2019, 2020 Alasdair Armstrong
// Copyright (c) 2020 Brian Campbell
//
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are
// met:
//
// 1. Redistributions of source code must retain the above copyright
// notice, this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright
// notice, this list of conditions and the following disclaimer in the
// documentation and/or other materials provided with the distribution.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
// "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
// LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
// HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
// SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
// LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
// DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
// THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

//! The memory is split up into various regions defined by a half-open
//! range between two addresses [base, top). This is done because we
//! want to give different semantics to various parts of memory,
//! e.g. program memory should be concrete, whereas the memory used
//! for loads and stores in litmus tests need to be totally symbolic
//! so the bevhaior can be imposed later as part of the concurrency
//! model.

use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use crate::concrete::BV;
use crate::error::ExecError;
use crate::ir::Val;
use crate::log;
use crate::smt::smtlib::{Def, Exp};
use crate::smt::{Event, Solver, Sym};

/// For now, we assume that we only deal with 64-bit architectures.
pub type Address = u64;

#[derive(Clone)]
pub enum Region<B> {
    /// A region with a symbolic value constrained by a symbolic
    /// variable generated by an arbitrary function. The region should
    /// return a bitvector variable representing the whole region, so
    /// in practice this should be used for small regions of memory.
    Constrained(Range<Address>, Arc<dyn Send + Sync + Fn(&mut Solver<B>) -> Sym>),
    /// A region of arbitrary symbolic locations
    Symbolic(Range<Address>),
    /// A read only region of arbitrary symbolic locations intended for code
    SymbolicCode(Range<Address>),
    /// A region of concrete read-only memory
    Concrete(Range<Address>, HashMap<Address, u8>)
}

pub enum SmtKind {
    ReadData,
    ReadInstr,
    WriteData,
}

impl<B> fmt::Debug for Region<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Region::*;
        match self {
            Constrained(r, _) => write!(f, "Constrained({:?}, <closure>)", r),
            Symbolic(r) => write!(f, "Symbolic({:?})", r),
            SymbolicCode(r) => write!(f, "SymbolicCode({:?})", r),
            Concrete(r, locs) => write!(f, "Concrete({:?}, {:?})", r, locs),
        }
    }
}

impl<B> Region<B> {
    fn region_range(&self) -> &Range<Address> {
        match self {
            Region::Constrained(r, _) => r,
            Region::Symbolic(r) => r,
            Region::SymbolicCode(r) => r,
            Region::Concrete(r, _) => r,
        }
    }
}

// Optional client interface.  At the time of writing this is only
// used by the test generation to enforce sequential memory, so we
// jump through a few hoops to avoid other clients seeing it.  If it
// was used more generally then it would be better to parametrise the
// Memory struct instead.

pub trait MemoryCallbacks<B>: fmt::Debug + MemoryCallbacksClone<B> + Send + Sync {
    fn symbolic_read(
        &self,
        regions: &[Region<B>],
        solver: &mut Solver<B>,
        value: &Val<B>,
        read_kind: &Val<B>,
        address: &Val<B>,
        bytes: u32,
    );
    #[allow(clippy::too_many_arguments)]
    fn symbolic_write(
        &mut self,
        regions: &[Region<B>],
        solver: &mut Solver<B>,
        value: Sym,
        write_kind: &Val<B>,
        address: &Val<B>,
        data: &Val<B>,
        bytes: u32,
    );
}

pub trait MemoryCallbacksClone<B> {
    fn clone_box(&self) -> Box<dyn MemoryCallbacks<B>>;
}

impl<B, T> MemoryCallbacksClone<B> for T
where
    T: 'static + MemoryCallbacks<B> + Clone,
{
    fn clone_box(&self) -> Box<dyn MemoryCallbacks<B>> {
        Box::new(self.clone())
    }
}

impl<B> Clone for Box<dyn MemoryCallbacks<B>> {
    fn clone(&self) -> Box<dyn MemoryCallbacks<B>> {
        self.clone_box()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Memory<B> {
    regions: Vec<Region<B>>,
    client_info: Option<Box<dyn MemoryCallbacks<B>>>,
}

impl<B: BV> Memory<B> {
    pub fn new() -> Self {
        Memory { regions: Vec::new(), client_info: None }
    }

    pub fn log(&self) {
        for region in &self.regions {
            match region {
                Region::Constrained(range, _) => {
                    log!(log::MEMORY, &format!("Memory range: [0x{:x}, 0x{:x}) constrained", range.start, range.end))
                }
                Region::Symbolic(range) => {
                    log!(log::MEMORY, &format!("Memory range: [0x{:x}, 0x{:x}) symbolic", range.start, range.end))
                }
                Region::SymbolicCode(range) => {
                    log!(log::MEMORY, &format!("Memory range: [0x{:x}, 0x{:x}) symbolic code", range.start, range.end))
                }
                Region::Concrete(range, _) => {
                    log!(log::MEMORY, &format!("Memory range: [0x{:x}, 0x{:x}) concrete", range.start, range.end))
                }
            }
        }
    }

    pub fn add_region(&mut self, region: Region<B>) {
        self.regions.push(region)
    }

    pub fn add_symbolic_region(&mut self, range: Range<Address>) {
        self.regions.push(Region::Symbolic(range))
    }

    pub fn add_symbolic_code_region(&mut self, range: Range<Address>) {
        self.regions.push(Region::SymbolicCode(range))
    }

    pub fn add_concrete_region(&mut self, range: Range<Address>, contents: HashMap<Address, u8>) {
        self.regions.push(Region::Concrete(range, contents))
    }

    pub fn set_client_info(&mut self, info: Box<dyn MemoryCallbacks<B>>) {
        self.client_info = Some(info);
    }

    pub fn write_byte(&mut self, address: Address, byte: u8) {
        for region in &mut self.regions {
            match region {
                Region::Concrete(range, contents) if range.contains(&address) => {
                    contents.insert(address, byte);
                    return;
                }
                _ => (),
            }
        }
        self.regions.push(Region::Concrete(address..address, vec![(address, byte)].into_iter().collect()))
    }

    /// Read from the memory region determined by the address. If the address is symbolic the read
    /// value is always also symbolic. The number of bytes must be concrete otherwise will return a
    /// SymbolicLength error.
    ///
    /// # Panics
    ///
    /// Panics if the number of bytes to read is concrete but does not fit
    /// in a u32, which should never be the case.
    pub fn read(
        &self,
        read_kind: Val<B>,
        address: Val<B>,
        bytes: Val<B>,
        solver: &mut Solver<B>,
    ) -> Result<Val<B>, ExecError> {
        log!(log::MEMORY, &format!("Read: {:?} {:?} {:?}", read_kind, address, bytes));

        if let Val::I128(bytes) = bytes {
            let bytes = u32::try_from(bytes).expect("Bytes did not fit in u32 in memory read");

            if let Val::Bits(concrete_addr) = address {
                for region in &self.regions {
                    match region {
                        Region::Constrained(range, generator) if range.contains(&concrete_addr.lower_u64()) => {
                            return read_constrained(
                                range,
                                generator.as_ref(),
                                read_kind,
                                concrete_addr.lower_u64(),
                                bytes,
                                solver,
                            )
                        }

                        Region::Symbolic(range) if range.contains(&concrete_addr.lower_u64()) => {
                            return self.read_symbolic(read_kind, address, bytes, solver)
                        }

                        Region::SymbolicCode(range) if range.contains(&concrete_addr.lower_u64()) => {
                            return self.read_symbolic(read_kind, address, bytes, solver)
                        }

                        Region::Concrete(range, contents) if range.contains(&concrete_addr.lower_u64()) => {
                            return read_concrete(contents, read_kind, concrete_addr.lower_u64(), bytes, solver)
                        }

                        _ => continue,
                    }
                }

                self.read_symbolic(read_kind, address, bytes, solver)
            } else {
                self.read_symbolic(read_kind, address, bytes, solver)
            }
        } else {
            Err(ExecError::SymbolicLength("read_symbolic"))
        }
    }

    pub fn write(
        &mut self,
        write_kind: Val<B>,
        address: Val<B>,
        data: Val<B>,
        solver: &mut Solver<B>,
    ) -> Result<Val<B>, ExecError> {
        log!(log::MEMORY, &format!("Write: write_kind={:?} address={:?} data={:?}", write_kind, address, data));

        if let Val::Bits(_) = address {
            self.write_symbolic(write_kind, address, data, solver)
        } else {
            self.write_symbolic(write_kind, address, data, solver)
        }
    }

    /// The simplest read is to symbolically read a memory location. In
    /// that case we just return a fresh SMT bitvector of the appropriate
    /// size, and add a ReadMem event to the trace. For this we need the
    /// number of bytes to be non-symbolic.
    fn read_symbolic(
        &self,
        read_kind: Val<B>,
        address: Val<B>,
        bytes: u32,
        solver: &mut Solver<B>,
    ) -> Result<Val<B>, ExecError> {
        use crate::smt::smtlib::*;

        let value = solver.fresh();
        solver.add(Def::DeclareConst(value, Ty::BitVec(8 * bytes)));
        match &self.client_info {
            Some(c) => c.symbolic_read(&self.regions, solver, &Val::Symbolic(value), &read_kind, &address, bytes),
            None => (),
        };
        log!(log::MEMORY, &format!("Read symbolic ({:?}): {}", &address, value));
        solver.add_event(Event::ReadMem { value: Val::Symbolic(value), read_kind, address, bytes });
        Ok(Val::Symbolic(value))
    }

    /// `write_symbolic` just adds a WriteMem event to the trace,
    /// returning a symbolic boolean (the semantics of which is controlled
    /// by a memory model if required, but can be ignored in
    /// others). Raises a type error if the data argument is not a
    /// bitvector with a length that is a multiple of 8. This should be
    /// guaranteed by the Sail type system.
    fn write_symbolic(
        &mut self,
        write_kind: Val<B>,
        address: Val<B>,
        data: Val<B>,
        solver: &mut Solver<B>,
    ) -> Result<Val<B>, ExecError> {
        use crate::smt::smtlib::*;

        let data_length = crate::primop::length_bits(&data, solver)?;
        if data_length % 8 != 0 {
            return Err(ExecError::Type("write_symbolic"));
        };
        let bytes = data_length / 8;

        let value = solver.fresh();
        solver.add(Def::DeclareConst(value, Ty::Bool));
        match &mut self.client_info {
            Some(c) => c.symbolic_write(&self.regions, solver, value, &write_kind, &address, &data, bytes),
            None => (),
        };
        solver.add_event(Event::WriteMem { value, write_kind, address, data, bytes });

        Ok(Val::Symbolic(value))
    }

    pub fn smt_address_constraint(&self, address: &Exp, bytes: u32, kind: SmtKind, solver: &mut Solver<B>) -> Exp {
        smt_address_constraint(&self.regions, address, bytes, kind, solver)
    }
}

pub fn smt_address_constraint<B: BV>(
    regions: &[Region<B>],
    address: &Exp,
    bytes: u32,
    kind: SmtKind,
    solver: &mut Solver<B>,
) -> Exp {
    use crate::smt::smtlib::Exp::*;
    let addr_var = match address {
        Var(v) => *v,
        _ => {
            let v = solver.fresh();
            solver.add(Def::DefineConst(v, address.clone()));
            v
        }
    };
    regions
        .iter()
        .filter(|r| match kind {
            SmtKind::ReadData => true,
            SmtKind::ReadInstr => match r {
                Region::SymbolicCode(_) => true,
                _ => false,
            },
            SmtKind::WriteData => match r {
                Region::Symbolic(_) => true,
                _ => false,
            },
        })
        .map(|r| r.region_range())
        .filter(|r| r.end - r.start >= bytes as u64)
        .map(|r| {
            And(
                Box::new(Bvule(Box::new(Bits64(r.start, 64)), Box::new(Var(addr_var)))),
                // Use an extra bit to prevent wrapping
                Box::new(Bvult(
                    Box::new(Bvadd(
                        Box::new(ZeroExtend(65, Box::new(Var(addr_var)))),
                        Box::new(ZeroExtend(65, Box::new(Bits64(bytes as u64, 64)))),
                    )),
                    Box::new(ZeroExtend(65, Box::new(Bits64(r.end, 64)))),
                )),
            )
        })
        .fold(Bool(false), |acc, e| match acc {
            Bool(false) => e,
            _ => Or(Box::new(acc), Box::new(e)),
        })
}

fn reverse_endianness(bytes: &mut [u8]) {
    if bytes.len() <= 2 {
        bytes.reverse()
    } else {
        let (bytes_upper, bytes_lower) = bytes.split_at_mut(bytes.len() / 2);
        reverse_endianness(bytes_upper);
        reverse_endianness(bytes_lower);
        bytes.rotate_left(bytes.len() / 2)
    }
}

fn read_constrained<B: BV>(
    range: &Range<Address>,
    generator: &(dyn Fn(&mut Solver<B>) -> Sym),
    read_kind: Val<B>,
    address: Address,
    bytes: u32,
    solver: &mut Solver<B>,
) -> Result<Val<B>, ExecError> {
    let region = generator(solver);
    if address == range.start && address + bytes as u64 == range.end {
        solver.add_event(Event::ReadMem {
            value: Val::Symbolic(region),
            read_kind,
            address: Val::Bits(B::from_u64(address)),
            bytes,
        });
        Ok(Val::Symbolic(region))
    } else {
        Err(ExecError::BadRead)
    }
}

fn read_concrete<B: BV>(
    region: &HashMap<Address, u8>,
    read_kind: Val<B>,
    address: Address,
    bytes: u32,
    solver: &mut Solver<B>,
) -> Result<Val<B>, ExecError> {
    let mut byte_vec: Vec<u8> = Vec::with_capacity(bytes as usize);
    for i in address..(address + u64::from(bytes)) {
        byte_vec.push(*region.get(&i).unwrap_or(&0))
    }

    reverse_endianness(&mut byte_vec);

    if byte_vec.len() <= 8 {
        log!(log::MEMORY, &format!("Read concrete ({:#018X}): {:?}", address, byte_vec));

        let value = Val::Bits(B::from_bytes(&byte_vec));
        solver.add_event(Event::ReadMem { value, read_kind, address: Val::Bits(B::from_u64(address)), bytes });
        Ok(Val::Bits(B::from_bytes(&byte_vec)))
    } else {
        // TODO: Handle reads > 64 bits
        Err(ExecError::BadRead)
    }
}
