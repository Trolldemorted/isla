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
use std::io::Write;
use std::process::exit;
use std::sync::Arc;
use std::time::Instant;

use isla_lib::concrete::bitvector129::B129;
use isla_lib::executor;
use isla_lib::executor::LocalFrame;
use isla_lib::init::{initialize_architecture, Initialized};
use isla_lib::ir::*;
use isla_lib::lexer::Lexer;
use isla_lib::value_parser::ValParser;
use isla_lib::zencode;
use isla_lib::{simplify, simplify::WriteOpts};
use isla_lib::smt::Event;

mod opts;
use opts::CommonOpts;

fn main() {
    let code = isla_main();
    unsafe { isla_lib::smt::finalize_solver() };
    exit(code)
}

#[allow(clippy::mutex_atomic)]
fn isla_main() -> i32 {
    let mut opts = opts::common_opts();
    opts.optopt("", "linear", "rewrite function into linear form", "<id>");
    opts.optflag("", "optimistic", "assume assertions succeed");
    opts.optflag("t", "traces", "print execution traces");
    opts.optflag("s", "simplify", "simplify function traces");

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

    let function_id = shared_state.symtab.lookup(&function_name);
    let (args, _, instrs) = shared_state.functions.get(&function_id).unwrap();

    let mut frame = LocalFrame::new(function_id, args, None, instrs);

    for (i, arg) in matches.free[1..].iter().enumerate() {
        if let Some((id, ty)) = args.get(i) {
            if arg != "_" {
                let val = ValParser::new().parse(Lexer::new(arg)).unwrap_or_else(|e| panic!("Unable to parse argument {}: {}", arg, e));
                val.plausible(ty, &shared_state.symtab).unwrap_or_else(|_| panic!("Bad initial value for {}", shared_state.symtab.to_str(*id)));
                frame.vars_mut().insert(*id, UVal::Init(val));
            }
        } else {
            eprintln!("Too many arguments");
            return 1;
        }
    }
    let task = frame.add_lets(&lets).add_regs(&regs).task(0);

    let queue = Arc::new(SegQueue::new());
    let now = Instant::now();
    executor::start_multi(num_threads, None, vec![task], &shared_state, queue.clone(), &executor::trace_value_collector);

    eprintln!("Execution took: {}ms", now.elapsed().as_millis());

    loop {
        match queue.pop() {
            Ok(Ok((_, result, mut events))) => {
                let stdout = std::io::stdout();
                let mut handle = stdout.lock();
                writeln!(handle, "Result: {}", result.to_string(&shared_state.symtab)).unwrap();
                if matches.opt_present("traces") {
                    if matches.opt_present("simplify") {
                        simplify::hide_initialization(&mut events);
                        simplify::remove_unused(&mut events);
                    }
                    let events: Vec<Event<B129>> = events.drain(..).rev().collect();
                    let write_opts = WriteOpts {
                        define_enum: !matches.opt_present("simplify"),
                        ..WriteOpts::default()
                    };
                    simplify::write_events_with_opts(&mut handle, &events, &shared_state.symtab, &write_opts).unwrap();
                }
            }
            // Error during execution
            Ok(Err(msg)) => {
                eprintln!("{}", msg);
                break 1;
            }
            // Empty queue
            Err(_) => break 0,
        }
    }
}
