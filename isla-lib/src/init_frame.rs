
fn initialize<'ir, 'task>(config_path: &PathBuf) -> LocalFrame<'ir, B64> {
    let config_file = config_path.join("aarch64.toml");
    let symtab_file = config_path.join("aarch64.symtab");
    let ir_file     = config_path.join("aarch64.irx");

    let strings: Vec<String> = bincode::deserialize(&fs::read(&symtab_file).unwrap()).unwrap();
    let symtab = Symtab::from_raw_table(&strings);

    let mut ir: Vec<Def<Name, B64>> =
        ir_serialize::deserialize(&fs::read(&ir_file).unwrap()).expect("Failed to deserialize IR");

    let isa_config: ISAConfig<B64> = ISAConfig::parse(&fs::read_to_string(&config_file).unwrap(), &symtab).unwrap();

    let Initialized { mut regs, lets, shared_state } =
        initialize_architecture(&mut ir, symtab, &isa_config, AssertionMode::Optimistic);

    // Initialize registers
    regs.insert(shared_state.symtab.lookup("z_PC"), UVal::Init(Val::Bits(B64::from_u64(0))));
    regs.insert(shared_state.symtab.lookup("zSP_EL0"), UVal::Init(Val::Bits(B64::from_u64(0x10000))));
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
    regs.insert(shared_state.symtab.lookup("z__highest_el_aarch32"), UVal::Init(Val::Bool(false)));
    regs.insert(shared_state.symtab.lookup("z_IRQPending"), UVal::Init(Val::Bool(false)));
    regs.insert(shared_state.symtab.lookup("z_FIQPending"), UVal::Init(Val::Bool(false)));
    // these are set in sail
    //regs.insert(shared_state.symtab.lookup("zCFG_ID_AA64PFR0_EL1_EL3"), UVal::Init(Val::Bits(B64::new(2, 4))));
    //regs.insert(shared_state.symtab.lookup("zCFG_ID_AA64PFR0_EL1_EL2"), UVal::Init(Val::Bits(B64::new(2, 4))));
    //regs.insert(shared_state.symtab.lookup("zCFG_ID_AA64PFR0_EL1_EL1"), UVal::Init(Val::Bits(B64::new(2, 4))));
    //regs.insert(shared_state.symtab.lookup("zCFG_ID_AA64PFR0_EL1_EL0"), UVal::Init(Val::Bits(B64::new(2, 4))));

    // PSTATE is a struct, so it needs special handling
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

    let function_id = shared_state.symtab.lookup("zStep_CPU");
    let (args, _, instrs) = shared_state.functions.get(&function_id).unwrap();
    let mut lf: LocalFrame<B64> = LocalFrame::new(function_id, args, None, instrs); // Some(&[Val::Bits(B64::from_u32(opcode))])
    lf.add_lets(&lets);
    lf.add_regs(&regs);

    // Initialize memory
    let mem = lf.memory_mut();
    mem.add_concrete_region(0..4096, HashMap::new());
    mem.write_byte(0, 0x1f);
    mem.write_byte(1, 0x20);
    mem.write_byte(2, 0x03);
    mem.write_byte(3, 0xd5);
    for i in 4..4096 {
        mem.write_byte(i, 0);
    }
    log::set_flags(0xffffffff);
    unimplemented!() // can't return because references -.-
    //lf
}