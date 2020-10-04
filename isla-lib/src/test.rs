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
use isla_lib::error::ExecError;
use isla_lib::smt::Event;
use isla_lib::log;
use isla_lib::executor::Backtrace;
use crossbeam::queue::SegQueue;

/*
fn initialize<'ir, 'task>() -> isla_lib::executor::Task<'ir, 'task, B64> {
    
}
*/

pub type SimpleResultQueue<'ir, B> = SegQueue<Result<LocalFrame<'ir, B>, (ExecError, Backtrace)>>;

fn simple_collector<'ir, B: BV>(
    _: usize,
    task_id: usize,
    result: Result<(Val<B>, LocalFrame<'ir, B>), (ExecError, Backtrace)>,
    shared_state: &SharedState<'ir, B>,
    solver: Solver<B>,
    collected: &SimpleResultQueue<'ir, B>,
) {
    let mut events = solver.trace().to_vec();
    match result {
        Ok((_, frame)) => {
            println!("collector got ok frame: {:?}", shared_state.symtab.to_str(frame.function_name));
            collected.push(Ok(frame))
        },
        Err(e) => collected.push(Err(e))
    }
}

fn step<'ir, 'task, B: BV>(
    task: isla_lib::executor::Task<'ir, 'task, B>,
    shared_state: &isla_lib::ir::SharedState<'ir, B>) {
    println!("--------------- begin step -----------------");
    let queue = Arc::new(SegQueue::new());
    executor::start_multi(
        1,
        None,
        vec![task],
        shared_state,
        queue.clone(),
        &simple_collector,
    );
    let mut successors = vec!();
    loop {
        match queue.pop() {
            Ok(Ok(local_frame)) => {
                println!("got successor");
                successors.push(local_frame);
            }
            Ok(Err((error, backtrace))) =>  {
                println!("queue got error: {}", error.to_string(&backtrace, &shared_state));
                break
            }
            Err(_) => {
                break
            }
        }
    }
    println!("--------------- end step -------------------");
    println!("{:?}", &queue);

    for successor in successors {
        println!("Successor:");
        print_register(&successor, &shared_state.symtab, "z_PC");
    }
}

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

    let Initialized { mut regs, lets, shared_state } =
        initialize_architecture(&mut ir, symtab, &isa_config, AssertionMode::Optimistic);
    
    //for (reg_name, reg_val) in regs {
    //    println!("{:?}={:?}", shared_state.symtab.to_str(reg_name), reg_val)
    //}
    //return;
    regs.insert(shared_state.symtab.lookup("z_PC"), UVal::Init(Val::Bits(B64::from_u64(0))));
    regs.insert(shared_state.symtab.lookup("zHCR_EL2"), UVal::Init(Val::Bits(B64::from_u64(0))));
    regs.insert(shared_state.symtab.lookup("zSCR_EL3"), UVal::Init(Val::Bits(B64::new(0, 32))));
    regs.insert(shared_state.symtab.lookup("zCNTKCTL_EL1"), UVal::Init(Val::Bits(B64::new(0, 32))));
    regs.insert(shared_state.symtab.lookup("zCNTHCTL_EL2"), UVal::Init(Val::Bits(B64::new(0, 32))));
    regs.insert(shared_state.symtab.lookup("zSCTLR_EL3"), UVal::Init(Val::Bits(B64::new(0, 64)))); // this is most likely invalid
    regs.insert(shared_state.symtab.lookup("zOSLSR_EL1"), UVal::Init(Val::Bits(B64::new(0, 64)))); // lock stuff
    regs.insert(shared_state.symtab.lookup("zOSDLR_EL1"), UVal::Init(Val::Bits(B64::new(0, 64)))); // double lock stuff
    regs.insert(shared_state.symtab.lookup("zEDSCR"), UVal::Init(Val::Bits(B64::new(0, 32))));
    regs.insert(shared_state.symtab.lookup("z__defaultRAM"), UVal::Init(Val::Bits(B64::new(4096, 56))));
    regs.insert(shared_state.symtab.lookup("zCNTCV"), UVal::Init(Val::Bits(B64::new(0, 64))));

    // these are set in sail
    //regs.insert(shared_state.symtab.lookup("zCFG_ID_AA64PFR0_EL1_EL3"), UVal::Init(Val::Bits(B64::new(2, 4))));
    //regs.insert(shared_state.symtab.lookup("zCFG_ID_AA64PFR0_EL1_EL2"), UVal::Init(Val::Bits(B64::new(2, 4))));
    //regs.insert(shared_state.symtab.lookup("zCFG_ID_AA64PFR0_EL1_EL1"), UVal::Init(Val::Bits(B64::new(2, 4))));
    //regs.insert(shared_state.symtab.lookup("zCFG_ID_AA64PFR0_EL1_EL0"), UVal::Init(Val::Bits(B64::new(2, 4))));

    regs.insert(shared_state.symtab.lookup("z__highest_el_aarch32"), UVal::Init(Val::Bool(false)));
    regs.insert(shared_state.symtab.lookup("z_IRQPending"), UVal::Init(Val::Bool(false)));
    regs.insert(shared_state.symtab.lookup("z_FIQPending"), UVal::Init(Val::Bool(false)));

    let mut pstate = HashMap::new();
    pstate.insert(shared_state.symtab.lookup("zN"), Val::Bits(B64::new(0, 1)));
    pstate.insert(shared_state.symtab.lookup("zZ"), Val::Bits(B64::new(0, 1)));
    pstate.insert(shared_state.symtab.lookup("zC"), Val::Bits(B64::new(0, 1)));
    pstate.insert(shared_state.symtab.lookup("zV"), Val::Bits(B64::new(0, 1)));
    pstate.insert(shared_state.symtab.lookup("zD"), Val::Bits(B64::new(0, 1)));
    pstate.insert(shared_state.symtab.lookup("zA"), Val::Bits(B64::new(0, 1)));
    pstate.insert(shared_state.symtab.lookup("zI"), Val::Bits(B64::new(0, 1)));
    pstate.insert(shared_state.symtab.lookup("zF"), Val::Bits(B64::new(0, 1)));
    pstate.insert(shared_state.symtab.lookup("zPAN"), Val::Bits(B64::new(0, 1)));
    pstate.insert(shared_state.symtab.lookup("zUAO"), Val::Bits(B64::new(0, 1)));
    pstate.insert(shared_state.symtab.lookup("zDIT"), Val::Bits(B64::new(0, 1)));
    pstate.insert(shared_state.symtab.lookup("zTCO"), Val::Bits(B64::new(0, 1)));
    pstate.insert(shared_state.symtab.lookup("zBTYPE"), Val::Bits(B64::new(0, 2)));
    pstate.insert(shared_state.symtab.lookup("zSS"), Val::Bits(B64::new(0, 1)));
    pstate.insert(shared_state.symtab.lookup("zIL"), Val::Bits(B64::new(0, 1)));
    pstate.insert(shared_state.symtab.lookup("zEL"), Val::Bits(B64::new(3, 2)));
    pstate.insert(shared_state.symtab.lookup("znRW"), Val::Bits(B64::new(0, 1)));
    pstate.insert(shared_state.symtab.lookup("zSP"), Val::Bits(B64::new(0, 1)));
    pstate.insert(shared_state.symtab.lookup("zQ"), Val::Bits(B64::new(0, 1)));
    pstate.insert(shared_state.symtab.lookup("zGE"), Val::Bits(B64::new(0, 4)));
    pstate.insert(shared_state.symtab.lookup("zSSBS"), Val::Bits(B64::new(0, 1)));
    pstate.insert(shared_state.symtab.lookup("zIT"), Val::Bits(B64::new(0, 8)));
    pstate.insert(shared_state.symtab.lookup("zJ"), Val::Bits(B64::new(0, 1)));
    pstate.insert(shared_state.symtab.lookup("zT"), Val::Bits(B64::new(0, 1)));
    pstate.insert(shared_state.symtab.lookup("zE"), Val::Bits(B64::new(0, 1)));
    pstate.insert(shared_state.symtab.lookup("zM"), Val::Bits(B64::new(0, 5)));

    regs.insert(shared_state.symtab.lookup("zPSTATE"), UVal::Init(Val::Struct(pstate)));
    
    //pc = &mut UVal::Init(Val::I64(0));

    let function_id = shared_state.symtab.lookup("zStep_CPU");
    let (args, _, instrs) = shared_state.functions.get(&function_id).unwrap();
    //let opcode: u32 = 0xd28065e1; // MOV X1, 815
    //0xd503201f // NO
    let mut lf: LocalFrame<B64> = LocalFrame::new(function_id, args, None, instrs); // Some(&[Val::Bits(B64::from_u32(opcode))])
    lf.add_lets(&lets);
    lf.add_regs(&regs);
    let mem = lf.memory_mut();
    //mem.write_byte(0, 0xE1);
    //mem.write_byte(1, 0x65);
    //mem.write_byte(2, 0x06);
    //mem.write_byte(3, 0x28);

    let mut region = HashMap::new();
    mem.add_concrete_region(0..4, region);

    mem.write_byte(0, 0x1f);
    mem.write_byte(1, 0x20);
    mem.write_byte(2, 0x03);
    mem.write_byte(3, 0xd5);

    //mem.write_byte(3, 0x1f);
    //mem.write_byte(2, 0x20);
    //mem.write_byte(1, 0x03);
    //mem.write_byte(0, 0xd5);

    for i in 4..4096 {
        mem.write_byte(i, 0);
    }
    let task = lf.task(0);
    log::set_flags(0xffffffff);
    step(task, &shared_state);
}

fn print_register<'ir, B: BV>(local_frame: &LocalFrame<'ir, B>, symtab: &Symtab, name: &str) {
    let x1 = symtab.get(name).unwrap();
    println!("{}={:?}", name, local_frame.regs().get(&x1));
}
