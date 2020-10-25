// BSD 2-Clause License
//
// Copyright (c) 2020 Alasdair Armstrong
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

use crossbeam::queue::SegQueue;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::convert::TryFrom;
use std::io::Write;
use std::process::exit;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

use isla_lib::concrete::bitvector129::B129;
use isla_lib::concrete::BV;
use isla_lib::error::ExecError;
use isla_lib::executor;
use isla_lib::executor::{Backtrace, LocalFrame};
use isla_lib::init::{initialize_architecture, Initialized};
use isla_lib::ir::*;
use isla_lib::lexer::Lexer;
use isla_lib::smt::smtlib::Exp;
use isla_lib::smt::{Event, Model, SmtResult, Solver};
use isla_lib::value_parser::ValParser;
use isla_lib::zencode;
use isla_lib::{log, log_from};
use isla_lib::{simplify, simplify::WriteOpts};

mod opts;
use opts::CommonOpts;

fn main() {
    let code = isla_main();
    unsafe { isla_lib::smt::finalize_solver() };
    exit(code)
}

fn parse_function_names<B: BV>(names: Vec<String>, shared_state: &SharedState<B>) -> HashSet<Name> {
    let mut set = HashSet::new();
    for f in names {
        let fz = zencode::encode(&f);
        let n = shared_state
            .symtab
            .get(&fz)
            .or_else(|| shared_state.symtab.get(&f))
            .unwrap_or_else(|| panic!("Function {} not found", f));
        set.insert(n);
    }
    set
}

#[allow(clippy::mutex_atomic)]
fn isla_main() -> i32 {
    let mut opts = opts::common_opts();
    opts.optopt("", "linear", "rewrite function into linear form", "<id>");
    opts.optflag("", "optimistic", "assume assertions succeed");
    opts.optflag("t", "traces", "print execution traces for successful executions");
    opts.optflag("", "error-traces", "print execution traces for paths that fail");
    opts.optflag("s", "simplify", "simplify function traces");
    opts.optflag("m", "model", "query SMT model to fill in variables");
    opts.optmulti("k", "stop-fn", "stop executions early if they reach this function", "<function name>");

    let mut hasher = Sha256::new();
    let (matches, arch) = opts::parse::<B129>(&mut hasher, &opts);
    let CommonOpts { num_threads, mut arch, symtab, isa_config } =
        opts::parse_with_arch(&mut hasher, &opts, &matches, &arch);

    if matches.free.len() == 0 {
        eprintln!("No function given");
        return 1;
    }
    let function_name = zencode::encode(&matches.free[0]);

    let assertion_mode =
        if matches.opt_present("optimistic") { AssertionMode::Optimistic } else { AssertionMode::Pessimistic };

    let Initialized { regs, lets, shared_state } =
        initialize_architecture(&mut arch, symtab, &isa_config, assertion_mode);

    let stop_functions = parse_function_names(matches.opt_strs("stop-fn"), &shared_state);
    let function_id = shared_state.symtab.lookup(&function_name);
    let (args, _, instrs) = shared_state.functions.get(&function_id).unwrap();

    let mut frame = LocalFrame::new(function_id, args, None, instrs);

    for (i, arg) in matches.free[1..].iter().enumerate() {
        if let Some((id, ty)) = args.get(i) {
            if arg.starts_with("_:") {
                let size = u32::from_str(&arg[2..]).unwrap_or_else(|_| panic!("Bad size in {}", arg));
                frame.vars_mut().insert(*id, UVal::Uninit(Box::leak(Box::new(Ty::Bits(size)))));
            } else if arg != "_" {
                let val = ValParser::new()
                    .parse(Lexer::new(arg))
                    .unwrap_or_else(|e| panic!("Unable to parse argument {}: {}", arg, e));
                let val = match (ty, val) {
                    (Ty::I64, Val::I128(i)) => {
                        let j = i64::try_from(i).unwrap();
                        Val::I64(j)
                    }
                    (_, v) => v,
                };
                val.plausible(ty, &shared_state.symtab)
                    .unwrap_or_else(|_| panic!("Bad initial value for {}", shared_state.symtab.to_str(*id)));
                frame.vars_mut().insert(*id, UVal::Init(val));
            }
        } else {
            eprintln!("Too many arguments");
            return 1;
        }
    }
    let mut task = frame.add_lets(&lets).add_regs(&regs).task(0);
    task.set_stop_functions(&stop_functions);

    let traces = matches.opt_present("traces");
    let error_traces = matches.opt_present("error-traces");
    let models = matches.opt_present("model");
    let collecting = Arc::new((SegQueue::new(), traces | error_traces, models));
    let now = Instant::now();
    executor::start_multi(num_threads, None, vec![task], &shared_state, collecting.clone(), &model_collector);

    eprintln!("Execution took: {}ms", now.elapsed().as_millis());

    let (queue, _, _) = collecting.as_ref();

    let write_events = |mut events, handle: &mut dyn Write| {
        if matches.opt_present("simplify") {
            simplify::hide_initialization(&mut events);
            simplify::remove_unused(&mut events);
        }
        let events: Vec<Event<B129>> = events.drain(..).rev().collect();
        let write_opts = WriteOpts { define_enum: !matches.opt_present("simplify"), ..WriteOpts::default() };
        simplify::write_events_with_opts(handle, &events, &shared_state.symtab, &write_opts).unwrap();
    };

    loop {
        match queue.pop() {
            Ok(Ok((_, result, events))) => {
                let stdout = std::io::stdout();
                let mut handle = stdout.lock();
                writeln!(handle, "Result: {}", result.to_string(&shared_state.symtab)).unwrap();
                if traces {
                    write_events(events, &mut handle);
                }
            }
            // Error during execution
            Ok(Err((msg, events))) => {
                let stdout = std::io::stdout();
                let mut handle = stdout.lock();
                writeln!(handle, "{}", msg).unwrap();
                if error_traces {
                    write_events(events, &mut handle);
                }
            }
            // Empty queue
            Err(_) => break 0,
        }
    }
}

fn bits_to_bv<B: BV>(bits: &[bool]) -> B {
    let mut bv = B::zeros(bits.len() as u32);
    for n in 0..bits.len() {
        if bits[n] {
            bv = bv.set_slice(n as u32, B::BIT_ONE);
        };
    }
    bv
}

fn concrete_value<B: BV>(model: &mut Model<B>, val: &Val<B>) -> Val<B> {
    match val {
        Val::Symbolic(v) => match model.get_var(*v) {
            Ok(Some(Exp::Bits64(result, size))) => Val::Bits(B::new(result, size)),
            Ok(Some(Exp::Bits(bs))) => Val::Bits(bits_to_bv(&bs)),
            _ => val.clone(),
        },
        Val::Vector(vec) => Val::Vector(vec.iter().map(|v| concrete_value(model, v)).collect()),
        Val::List(vec) => Val::List(vec.iter().map(|v| concrete_value(model, v)).collect()),
        Val::Struct(map) => Val::Struct(map.iter().map(|(k, v)| (*k, concrete_value(model, v))).collect()),
        Val::Ctor(n, v) => Val::Ctor(*n, Box::new(concrete_value(model, v))),
        _ => val.clone(),
    }
}

type AllTraceValueQueue<B> = SegQueue<Result<(usize, Val<B>, Vec<Event<B>>), (String, Vec<Event<B>>)>>;

fn model_collector<'ir, B: BV>(
    tid: usize,
    task_id: usize,
    result: Result<(Val<B>, LocalFrame<'ir, B>), (ExecError, Backtrace)>,
    shared_state: &SharedState<'ir, B>,
    mut solver: Solver<B>,
    (collected, trace, models): &(AllTraceValueQueue<B>, bool, bool),
) {
    let events: Vec<Event<B>> = if *trace { solver.trace().to_vec().drain(..).cloned().collect() } else { vec![] };
    match result {
        Ok((val, _)) => {
            if solver.check_sat() == SmtResult::Sat {
                let val = if *models {
                    let mut model = Model::new(&solver);
                    concrete_value(&mut model, &val)
                } else {
                    val
                };
                collected.push(Ok((task_id, val, events)))
            } else {
                collected.push(Err((format!("Got value {} but unsat?", val.to_string(&shared_state.symtab)), events)))
            }
        }
        Err((ExecError::Dead, _)) => (),
        Err((err, backtrace)) => {
            log_from!(tid, log::VERBOSE, format!("Error {:?}", err));
            for (f, pc) in backtrace.iter().rev() {
                log_from!(tid, log::VERBOSE, format!("  {} @ {}", shared_state.symtab.to_str(*f), pc));
            }
            if solver.check_sat() == SmtResult::Sat {
                let model = Model::new(&solver);
                collected.push(Err((format!("Error {:?}\n{:?}", err, model), events)))
            } else {
                collected.push(Err((format!("Error {:?}\nno model", err), events)))
            }
        }
    }
}
