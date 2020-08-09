use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use std::collections::HashMap;
use std::sync::Arc;
use isla_lib::concrete::{bitvector64::B64, BV};
use isla_lib::memory::Memory;
use isla_lib::init::{initialize_architecture, Initialized};
use isla_lib::ir::serialize as ir_serialize;
use isla_lib::ir::*;
use isla_lib::executor::LocalFrame;
use isla_lib::executor;
use isla_lib::config::ISAConfig;
use isla_lib::smt::Config;
use isla_lib::smt::Context;
use isla_lib::smt::Solver;
use crossbeam::queue::SegQueue;

fn main() {
    let now = Instant::now();
    let config_file = PathBuf::from(r"C:\Users\Benni\Downloads\aarch64.toml");
    let symtab_file = PathBuf::from(r"C:\Users\Benni\Downloads\aarch64.symtab");
    let ir_file     = PathBuf::from(r"C:\Users\Benni\Downloads\aarch64.irx");

    let strings: Vec<String> = bincode::deserialize(&fs::read(&symtab_file).unwrap()).unwrap();
    let symtab = Symtab::from_raw_table(&strings);
    let symtab1 = Symtab::from_raw_table(&strings);

    let mut ir: Vec<Def<Name, B64>> =
        ir_serialize::deserialize(&fs::read(&ir_file).unwrap()).expect("Failed to deserialize IR");

    let isa_config: ISAConfig<B64> = ISAConfig::parse(&fs::read_to_string(&config_file).unwrap(), &symtab).unwrap();

    println!("Loaded architecture in: {}ms", now.elapsed().as_millis());

    let Initialized { regs, lets, shared_state } =
        initialize_architecture(&mut ir, symtab, &isa_config, AssertionMode::Optimistic);

    let function_id = shared_state.symtab.lookup("zStep_System");
    let (args, _, instrs) = shared_state.functions.get(&function_id).unwrap();
    let opcode: u32 = 0xd28065e1; // MOV X1, 815
    let mut lf: LocalFrame<B64> = LocalFrame::new(function_id, args, None, instrs); // Some(&[Val::Bits(B64::from_u32(opcode))])
    lf.add_lets(&lets);
    lf.add_regs(&regs);
    let mem = lf.memory_mut();
    //mem.write_byte(0, 0xE1);
    //mem.write_byte(1, 0x65);
    //mem.write_byte(2, 0x06);
    //mem.write_byte(3, 0x28);
    let task = lf.task(0);
    let queue = Arc::new(SegQueue::new());
    println!("------------------------------------");
    executor::start_multi(
        1,
        None,
        vec![task],
        &shared_state,
        queue.clone(),
        &executor::trace_result_collector,
    );
    loop {
        match queue.pop() {
            Ok(Ok((x, result, mut events))) => {
            }
            Ok(Err(msg)) =>  {
                println!("Error: {}", msg);
                break
            }
            Err(_) => {
                break
            }
        }
    }
    let x1 = symtab1.get("zR1").unwrap();
    println!("zR1={:?}", regs.get(&x1));
}
