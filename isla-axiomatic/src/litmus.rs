// BSD 2-Clause License
//
// Copyright (c) 2019, 2020 Alasdair Armstrong
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

use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use toml::{Value, value::Table};

use isla_lib::concrete::BV;
use isla_lib::config::ISAConfig;
use isla_lib::ir::{Name, Symtab};
use isla_lib::log;
use isla_lib::memory::Region;
use isla_lib::smt::Solver;
use isla_lib::zencode;

use crate::sandbox::SandboxedCommand;

pub mod exp;
mod exp_lexer;
lalrpop_mod!(#[allow(clippy::all)] exp_parser, "/litmus/exp_parser.rs");

/// We have a special purpose temporary file module which is used to
/// create the output file for each assembler/linker invocation. Each
/// call to new just creates a new file name using our PID and a
/// unique counter. This file isn't opened until we read it, after the
/// assembler has created the object file. Dropping the `TmpFile`
/// removes the file if it exists.
mod tmpfile {
    use std::env;
    use std::fs::{create_dir, remove_file, OpenOptions};
    use std::io::prelude::*;
    use std::path::{Path, PathBuf};
    use std::process;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    pub struct TmpFile {
        path: PathBuf,
    }

    static TMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    impl TmpFile {
        pub fn new() -> TmpFile {
            let mut path = env::temp_dir();
            path.push("isla");
            if !path.is_dir() {
                create_dir(&path).expect("Could not create temporary directory")
            }
            path.push(format!("isla_{}_{}", process::id(), TMP_COUNTER.fetch_add(1, Ordering::SeqCst)));
            TmpFile { path }
        }

        pub fn path(&self) -> &Path {
            self.path.as_ref()
        }

        pub fn read_to_end(&mut self) -> std::io::Result<Vec<u8>> {
            let mut fd = OpenOptions::new().read(true).open(&self.path)?;
            let mut buffer = Vec::new();
            fd.read_to_end(&mut buffer)?;
            Ok(buffer)
        }
    }

    impl Drop for TmpFile {
        fn drop(&mut self) {
            if remove_file(&self.path).is_err() {}
        }
    }
}

type ThreadName = String;

/// In addition to the threads, system litmus tests can contain extra
/// sections containing additional code. These are linked at specific
/// addresess. For example we might place a section at VBAR_EL1 for a
/// thread to serve as an exception handler in ARMv8.
struct UnassembledSection<'a> {
    name: &'a str,
    address: u64,
    code: &'a str,
}

static THREAD_PREFIX: &str = "litmus_";

fn validate_section_name(name: &str) -> bool {
    for (i, c) in name.chars().enumerate() {
        if i == 0 && !c.is_ascii_alphabetic() {
            return false
        }
        
        if !(c.is_ascii_alphanumeric() || c == '_') {
            return false
        }
    }

    // Would conflict with the name we use for threads by default
    if name.len() >= THREAD_PREFIX.len() && &name[0..THREAD_PREFIX.len()] == THREAD_PREFIX {
        return false
    }

    true
}

fn parse_address(addr: &str) -> Result<u64, String> {
    if addr.len() < 2 {
        return Err(format!("Address {} is too short, it must have the form 0xHEX or #xHEX", addr))
    }
    if &addr[0..2] != "0x" && &addr[0..2] != "#x" {
        return Err(format!("Address {} must start with either `0x' or `#x'", addr))
    }
    u64::from_str_radix(&addr[2..], 16).map_err(|_| format!("Cannot parse {} as hexadecimal", addr))
}

enum LinkerLine<'a, 'b> {
    Thread(&'a str),
    Section(&'a UnassembledSection<'b>)
}

/// When we assemble a litmus test, we need to make sure any branch
/// instructions have addresses that will match the location at which
/// we load each thread in memory. To do this we invoke the linker and
/// give it a linker script with the address for each thread in the
/// litmus thread.
fn generate_linker_script<B>(
    threads: &[(ThreadName, &str)],
    sections: &[UnassembledSection<'_>],
    isa: &ISAConfig<B>
) -> String {
    use std::fmt::Write;
    use LinkerLine::*;
    
    let mut thread_address = isa.thread_base;

    let mut script = String::new();
    writeln!(&mut script, "start = 0;\nSECTIONS\n{{").unwrap();

    let mut t = 0;
    let mut s = 0;

    loop {
        let line = match (threads.get(t), sections.get(s)) {
            (Some((tid, _)), Some(section)) if thread_address < section.address => Thread(&*tid),
            (Some(_), Some(section)) => Section(section),
            (Some((tid, _)), None) => Thread(&*tid),
            (None, Some(section)) => Section(section),
            (None, None) => break,
        };

        match line {
            Thread(tid) => {
                writeln!(&mut script, "  . = 0x{:x};\n  {}{} : {{ *({}{}) }}", thread_address, THREAD_PREFIX, tid, THREAD_PREFIX, tid).unwrap();
                thread_address += isa.thread_stride;
                t += 1
            }
            Section(section) => {
                writeln!(&mut script, "  . = 0x{:x};\n  {} : {{ *({}) }}", section.address, section.name, section.name).unwrap();
                s += 1
            }
        }
    }

    writeln!(&mut script, "}}").unwrap();

    log!(log::LITMUS, script);
    
    script
}

type AssembledThreads = (Vec<(ThreadName, Vec<u8>)>, String);

#[cfg(feature = "sandbox")]
fn validate_code(code: &str) -> Result<(), String> {
    // We already run in sandbox, but we can additionally rule out any
    // directives
    if code.contains('.') {
        return Err("Invalid assembly in litmus".to_string());
    }

    if code.len() > 1000 {
        return Err("Assembly in litmus thread too long".to_string());
    }

    for c in code.chars() {
        if !c.is_ascii() || (c.is_control() && !c.is_ascii_whitespace()) {
            return Err("Assembly block can contain only ascii text".to_string());
        }
    }

    Ok(())
}

#[cfg(not(feature = "sandbox"))]
fn validate_code(_: &str) -> Result<(), String> {
    Ok(())
}

/// This function takes some assembly code for each thread, which
/// should ideally be formatted as instructions separated by a newline
/// and a tab (`\n\t`), and invokes the assembler provided in the
/// `ISAConfig<B>` on this code. The generated ELF is then read in and
/// the assembled code is returned as a vector of bytes corresponding
/// to it's section in the ELF file as given by the thread name. If
/// `reloc` is true, then we will also invoke the linker to place each
/// thread's section at the correct address.
fn assemble<B>(
    threads: &[(ThreadName, &str)],
    sections: &[UnassembledSection<'_>],
    reloc: bool,
    isa: &ISAConfig<B>
) -> Result<AssembledThreads, String> {
    use goblin::Object;

    let objfile = tmpfile::TmpFile::new();

    let mut assembler = SandboxedCommand::from_tool(&isa.assembler)
        .arg("-o")
        .arg(objfile.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .or_else(|err| {
            Err(format!("Failed to spawn assembler {}. Got error: {}", &isa.assembler.executable.display(), err))
        })?;

    // Write each thread to the assembler's standard input, in a section called `THREAD_PREFIXN` for each thread `N`
    {
        let stdin = assembler.stdin.as_mut().ok_or_else(|| "Failed to open stdin for assembler".to_string())?;
        for (thread_name, code) in threads.iter() {
            validate_code(code)?;
            stdin
                .write_all(format!("\t.section {}{}\n", THREAD_PREFIX, thread_name).as_bytes())
                .and_then(|_| stdin.write_all(code.as_bytes()))
                .or_else(|_| Err(format!("Failed to write to assembler input file {}", objfile.path().display())))?
        }
        for section in sections {
            validate_code(section.code)?;
            if !validate_section_name(section.name) { return Err(format!("Section name {} is invalid", section.name)) };
            stdin
                .write_all(format!("\t.section {}\n", section.name).as_bytes())
                .and_then(|_| stdin.write_all(section.code.as_bytes()))
                .or_else(|_| Err(format!("Failed to write to assembler input file {}", objfile.path().display())))?
        }
    }

    let output = assembler.wait_with_output().or_else(|_| Err("Failed to read stdout from assembler".to_string()))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let (mut objfile, objdump) = if reloc {
        let objfile_reloc = tmpfile::TmpFile::new();
        let linker_script = tmpfile::TmpFile::new();
        {
            let mut fd = File::create(linker_script.path())
                .or_else(|_| Err("Failed to create temp file for linker script".to_string()))?;
            fd.write_all(generate_linker_script(threads, sections, isa).as_bytes())
                .or_else(|_| Err("Failed to write linker script".to_string()))?;
        }

        let linker_status = SandboxedCommand::from_tool(&isa.linker)
            .arg("-T")
            .arg(linker_script.path())
            .arg("-o")
            .arg(objfile_reloc.path())
            .arg(objfile.path())
            .status()
            .map_err(|err| {
                format!("Failed to invoke linker {}. Got error: {}", &isa.linker.executable.display(), err)
            })?;

        // Invoke objdump to get the assembled output in human readable
        // form. If objdump fails for whatever reason, we don't want to
        // consider it a hard error however.
        let objdump = {
            let output = SandboxedCommand::from_tool(&isa.objdump).arg("-D").arg(objfile_reloc.path()).output();

            if let Ok(output) = output {
                String::from_utf8_lossy(if output.status.success() { &output.stdout } else { &output.stderr })
                    .to_string()
            } else {
                format!("Failed to invoke {}", &isa.objdump.executable.display())
            }
        };

        if linker_status.success() {
            (objfile_reloc, objdump)
        } else {
            return Err(format!("Linker failed with exit code {}", linker_status));
        }
    } else {
        (objfile, "Objdump not available unless linker was used".to_string())
    };

    let buffer = objfile.read_to_end().map_err(|_| "Failed to read generated ELF file".to_string())?;

    // Get the code from the generated ELF's `THREAD_PREFIXN` section for each thread
    let mut assembled: Vec<(ThreadName, Vec<u8>)> = Vec::new();
    match Object::parse(&buffer) {
        Ok(Object::Elf(elf)) => {
            let shdr_strtab = elf.shdr_strtab;
            for section in elf.section_headers {
                if let Some(Ok(section_name)) = shdr_strtab.get(section.sh_name) {
                    for (thread_name, _) in threads.iter() {
                        if section_name == format!("{}{}", THREAD_PREFIX, thread_name) {
                            let offset = section.sh_offset as usize;
                            let size = section.sh_size as usize;
                            assembled.push((thread_name.to_string(), buffer[offset..(offset + size)].to_vec()))
                        }
                    }
                }
            }
        }
        Ok(_) => return Err("Generated object was not an ELF file".to_string()),
        Err(err) => return Err(format!("Failed to parse ELF file: {}", err)),
    };

    if assembled.len() != threads.len() {
        return Err("Could not find all threads in generated ELF file".to_string());
    };

    log!(log::LITMUS, objdump);

    Ok((assembled, objdump))
}

/// For error reporting it's very helpful to be able to turn the raw
/// opcodes we work with into actual human-readable assembly. To do
/// this we use a regex to pair up the opcode with it's disassembly in
/// objdump output for the litmus test.
pub fn instruction_from_objdump<'obj>(opcode: &str, objdump: &'obj str) -> Option<String> {
    use regex::Regex;
    let instr_re = Regex::new(&format!(r"[0-9a-zA-Z]+:\s0*{}\s+(.*)", opcode)).unwrap();

    // Find all instructions for an opcode in the objdump output. Return None if
    // for some reason they are non-unique
    // (this could happen if e.g. relocations have not been applied tojumps).
    let mut instr: Option<&'obj str> = None;
    for caps in instr_re.captures_iter(objdump) {
        if let Some(prev) = instr {
            if prev == caps.get(1)?.as_str().trim() {
                continue;
            } else {
                return None;
            }
        } else {
            instr = Some(caps.get(1)?.as_str().trim())
        }
    }

    let whitespace_re = Regex::new(r"\s+").unwrap();
    Some(whitespace_re.replace_all(instr?, " ").to_string())
}

pub fn opcode_from_objdump<B: BV>(addr: B, objdump: &str) -> Option<B> {
    use regex::Regex;
    let opcode_re = Regex::new(&format!(r"{:x}:\t([0-9a-fA-F]+) \t", addr)).unwrap();

    if let Some(caps) = opcode_re.captures(objdump) {
        B::from_str(&format!("0x{}", caps.get(1)?.as_str()))
    } else {
        None
    }
}

fn label_from_objdump(label: &str, objdump: &str) -> Option<u64> {
    use regex::Regex;
    let label_re = Regex::new(&format!(r"([0-9a-fA-F]+) <{}>:", label)).unwrap();

    if let Some(caps) = label_re.captures(objdump) {
        u64::from_str_radix(caps.get(1)?.as_str(), 16).ok()
    } else {
        None
    }
}

pub fn assemble_instruction<B>(instr: &str, isa: &ISAConfig<B>) -> Result<Vec<u8>, String> {
    let instr = instr.to_owned() + "\n";
    if let [(_, bytes)] = assemble(&[("single".to_string(), &instr)], &[], false, isa)?.0.as_slice() {
        Ok(bytes.to_vec())
    } else {
        Err(format!("Failed to assemble instruction {}", instr))
    }
}

fn parse_symbolic_locations(
    litmus_toml: &Value,
    symbolic_addrs: &HashMap<String, u64>,
) -> Result<HashMap<String, u64>, String> {
    let sym_locs_table = match litmus_toml.get("locations") {
        Some(value) => value
            .as_table()
            .ok_or_else(|| "[locations] must be a table of <symbolic address> = <value> pairs".to_string())?,
        // Most litmus tests won't define any symbolic locations.
        None => return Ok(HashMap::new()),
    };

    let mut sym_locs = HashMap::new();
    for (sym_loc, value) in sym_locs_table {
        let value = value.as_str().ok_or_else(|| "Invalid symbolic address value")?;
        let value = match i64::from_str_radix(value, 10) {
            Ok(n) => n as u64,
            Err(_) => *symbolic_addrs.get(value).ok_or_else(|| {
                format!("Could not parse symbolic location value {} as an integer or address value", value)
            })?,
        };
        sym_locs.insert(sym_loc.clone(), value);
    }

    Ok(sym_locs)
}

fn parse_symbolic_types(litmus_toml: &Value) -> Result<HashMap<String, u32>, String> {
    let sym_types_table = match litmus_toml.get("types") {
        Some(value) => value
            .as_table()
            .ok_or_else(|| "[types] must be a table of <symbolic address> = <type> pairs".to_string())?,
        // Most litmus tests won't define any symbolic types.
        None => return Ok(HashMap::new()),
    };

    let mut sym_sizeof = HashMap::new();
    for (sym_type, ty) in sym_types_table {
        let sizeof = match ty.as_str() {
            Some("uint64_t") => 8,
            Some("uint32_t") => 4,
            Some("uint16_t") => 2,
            Some("uint8_t") => 1,
            _ => return Err("Invalid type in litmus [types] table".to_string()),
        };
        sym_sizeof.insert(sym_type.clone(), sizeof);
    }

    Ok(sym_sizeof)
}

fn parse_init<B>(
    reg: &str,
    value: &Value,
    symbolic_addrs: &HashMap<String, u64>,
    objdump: &str,
    symtab: &Symtab,
    isa: &ISAConfig<B>,
) -> Result<(Name, u64), String> {
    let reg = match isa.register_renames.get(reg) {
        Some(reg) => *reg,
        None => symtab.get(&zencode::encode(reg)).ok_or_else(|| format!("No register {} in thread init", reg))?,
    };

    let value = value.as_str().ok_or_else(|| "Init value must be a string".to_string())?;

    match symbolic_addrs.get(value) {
        Some(addr) => Ok((reg, *addr)),
        None => {
            if value.starts_with("0x") {
                match u64::from_str_radix(&value[2..], 16) {
                    Ok(n) => Ok((reg, n)),
                    Err(_) => Err(format!("Cannot parse hexadecimal initial value in litmus: {}", value)),
                }
            } else if value.ends_with(':') {
                match label_from_objdump(&value[0..value.len() - 1], objdump) {
                    Some(addr) => Ok((reg, addr)),
                    None => Err(format!("Could not find label {}", value)),
                }
            } else {
                match i64::from_str_radix(value, 10) {
                    Ok(n) => Ok((reg, n as u64)),
                    Err(_) => Err(format!("Cannot handle initial value in litmus: {}", value)),
                }
            }
        }
    }
}

fn parse_thread_inits<'a, B>(
    thread: &'a Value,
    symbolic_addrs: &HashMap<String, u64>,
    objdump: &str,
    symtab: &Symtab,
    isa: &ISAConfig<B>,
) -> Result<Vec<(Name, u64)>, String> {
    let inits = thread
        .get("init")
        .and_then(Value::as_table)
        .ok_or_else(|| "Thread init must be a list of register name/value pairs".to_string())?;

    inits
        .iter()
        .map(|(reg, value)| parse_init(reg, value, symbolic_addrs, objdump, symtab, isa))
        .collect::<Result<_, _>>()
}

fn parse_self_modify_region<B: BV>(toml_region: &Value, objdump: &str) -> Result<Region<B>, String> {
    let table = toml_region.as_table().ok_or_else(|| "Each self_modify element must be a TOML table".to_string())?;
    let address = table
        .get("address")
        .and_then(Value::as_str)
        .ok_or_else(|| "self_modify element must have a `address` field".to_string())?;
    let address = label_from_objdump(&address[0..(address.len() - 1)], objdump)
        .ok_or_else(|| "address not parseable in self_modify element")?;

    let bytes = table
        .get("bytes")
        .and_then(Value::as_integer)
        .ok_or_else(|| "self_modify element must have a `bytes` field".to_string())?;
    let upper = address + (bytes as u64);

    let values = table
        .get("values")
        .and_then(Value::as_array)
        .ok_or_else(|| "self_modify element must have a `values` field".to_string())?;
    let values = values
        .iter()
        .map(|v| v.as_str().and_then(B::from_str).map(|bv| (bv.lower_u64(), bv.len())))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| "Could not parse `values` field")?;

    Ok(Region::Constrained(
        address..upper,
        Arc::new(move |solver: &mut Solver<B>| {
            use isla_lib::smt::smtlib::{Def, Exp, Ty};
            let v = solver.fresh();
            let exp: Exp = values.iter().fold(Exp::Bool(false), |exp, (bits, len)| {
                Exp::Or(Box::new(Exp::Eq(Box::new(Exp::Var(v)), Box::new(Exp::Bits64(*bits, *len)))), Box::new(exp))
            });
            solver.add(Def::DeclareConst(v, Ty::BitVec(bytes as u32 * 8)));
            solver.add(Def::Assert(exp));
            v
        }),
    ))
}

fn parse_self_modify<B: BV>(toml: &Value, objdump: &str) -> Result<Vec<Region<B>>, String> {
    if let Some(value) = toml.get("self_modify") {
        let array = value.as_array().ok_or_else(|| "self_modify section must be a TOML array".to_string())?;
        Ok(array.iter().map(|v| parse_self_modify_region(v, objdump)).collect::<Result<_, _>>()?)
    } else {
        Ok(Vec::new())
    }
}

fn parse_extra<'v>(extra: (&'v String, &'v Value)) -> Result<UnassembledSection<'v>, String> {
    let addr = extra.1.get("address").and_then(|addr| addr.as_str()).ok_or_else(|| format!("No address in {}", extra.0))?;
    let code = extra.1.get("code").and_then(|code| code.as_str()).ok_or_else(|| format!("No code in {}", extra.0))?;
    Ok(UnassembledSection {
        name: &extra.0,
        address: parse_address(addr)?,
        code,
    })
}

pub type AssembledThread = (ThreadName, Vec<(Name, u64)>, Vec<u8>);

pub struct Litmus<B> {
    pub name: String,
    pub hash: Option<String>,
    pub symbolic_addrs: HashMap<String, u64>,
    pub symbolic_locations: HashMap<String, u64>,
    pub symbolic_sizeof: HashMap<String, u32>,
    pub assembled: Vec<AssembledThread>,
    pub self_modify_regions: Vec<Region<B>>,
    pub objdump: String,
    pub final_assertion: exp::Exp,
}

impl<B: BV> Litmus<B> {
    pub fn log(&self) {
        log!(log::LITMUS, &format!("Litmus test name: {}", self.name));
        log!(log::LITMUS, &format!("Litmus test hash: {:?}", self.hash));
        log!(log::LITMUS, &format!("Litmus test symbolic addresses: {:?}", self.symbolic_addrs));
        log!(log::LITMUS, &format!("Litmus test data: {:#?}", self.assembled));
        log!(log::LITMUS, &format!("Litmus test final assertion: {:?}", self.final_assertion));
    }

    pub fn parse(contents: &str, symtab: &Symtab, isa: &ISAConfig<B>) -> Result<Self, String> {
        let litmus_toml = match contents.parse::<Value>() {
            Ok(toml) => toml,
            Err(e) => return Err(format!("Error when parsing litmus: {}", e)),
        };

        let name = litmus_toml
            .get("name")
            .and_then(|n| n.as_str().map(str::to_string))
            .ok_or_else(|| "No name found in litmus file".to_string())?;

        let hash = litmus_toml.get("hash").map(|h| h.to_string());

        let symbolic = litmus_toml
            .get("symbolic")
            .and_then(Value::as_array)
            .ok_or("No symbolic addresses found in litmus file")?;
        let symbolic_addrs = symbolic
            .iter()
            .enumerate()
            .map(|(i, sym_addr)| match sym_addr.as_str() {
                Some(sym_addr) => {
                    Ok((sym_addr.to_string(), isa.symbolic_addr_base + (i as u64 * isa.symbolic_addr_stride)))
                }
                None => Err("Symbolic addresses must be strings"),
            })
            .collect::<Result<_, _>>()?;

        let symbolic_locations = parse_symbolic_locations(&litmus_toml, &symbolic_addrs)?;
        let symbolic_sizeof = parse_symbolic_types(&litmus_toml)?;

        let threads = litmus_toml.get("thread").and_then(|t| t.as_table()).ok_or("No threads found in litmus file")?;

        let code: Vec<(ThreadName, &str)> = threads
            .iter()
            .map(|(thread_name, thread)| {
                thread
                    .get("code")
                    .and_then(|code| code.as_str().map(|code| (thread_name.to_string(), code)))
                    .ok_or_else(|| format!("No code found for thread {}", thread_name))
            })
            .collect::<Result<_, _>>()?;

        let empty_table = toml::value::Map::new();
        let sections: &Table = litmus_toml.get("section").and_then(|t| t.as_table()).unwrap_or_else(|| &empty_table);
        let mut sections: Vec<UnassembledSection<'_>> = sections
            .iter()
            .map(parse_extra)
            .collect::<Result<_, _>>()?;
        sections.sort_unstable_by_key(|section| section.address);
        
        let (mut assembled, objdump) = assemble(&code, &sections, true, isa)?;

        let mut inits: Vec<Vec<(Name, u64)>> = threads
            .iter()
            .map(|(_, thread)| parse_thread_inits(thread, &symbolic_addrs, &objdump, symtab, isa))
            .collect::<Result<_, _>>()?;

        let assembled = assembled
            .drain(..)
            .zip(inits.drain(..))
            .map(|((thread_name, code), init)| (thread_name, init, code))
            .collect();

        let self_modify_regions = parse_self_modify::<B>(&litmus_toml, &objdump)?;

        let fin = litmus_toml.get("final").ok_or("No final section found in litmus file")?;
        let final_assertion = (match fin.get("assertion").and_then(Value::as_str) {
            Some(assertion) => {
                let lexer = exp_lexer::ExpLexer::new(&assertion);
                exp_parser::ExpParser::new().parse(&symbolic_addrs, &symbolic_sizeof, symtab, &isa.register_renames, lexer)
                    .map_err(|error| error.to_string())
            },
            None => Err("No final.assertion found in litmus file".to_string()),
        })?;

        Ok(Litmus {
            name,
            hash,
            symbolic_addrs,
            symbolic_locations,
            symbolic_sizeof,
            assembled,
            self_modify_regions,
            objdump,
            final_assertion,
        })
    }

    pub fn from_file<P>(path: P, symtab: &Symtab, isa: &ISAConfig<B>) -> Result<Self, String>
    where
        P: AsRef<Path>,
    {
        let mut contents = String::new();
        match File::open(&path) {
            Ok(mut handle) => match handle.read_to_string(&mut contents) {
                Ok(_) => (),
                Err(e) => return Err(format!("Unexpected failure while reading litmus: {}", e)),
            },
            Err(e) => return Err(format!("Error when loading litmus '{}': {}", path.as_ref().display(), e)),
        };

        Self::parse(&contents, symtab, isa)
    }
}
