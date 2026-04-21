#![allow(dead_code)]

mod ast;
mod backend_wasm;
#[allow(unused)]
mod lint;
mod lsp;
mod backend_x64;
mod codegen;
mod driver;
mod error;
mod ir;
mod ir_opt;
mod lexer;
mod lower;
mod optimize;
mod regalloc;
mod ssa;
mod wasm_encode;
mod parser;
mod peephole;
mod preprocess;

use std::env;
use std::process;

fn usage() -> ! {
    eprintln!("Usage: rcc [ -o <path> ] [ -S ] [ -E ] [ -c ] <file>");
    process::exit(1);
}

struct Args {
    input: String,
    output: Option<String>,
    emit_asm: bool,       // -S: output .s only
    preprocess_only: bool, // -E: preprocess only
    compile_only: bool,    // -c: compile to .o
    dump_ast: bool,
    dump_ir: bool,         // --ir: dump IR
    use_ir: bool,          // --use-ir: use IR pipeline
    emit_wasm: bool,       // --wasm: emit WAT
    explain: bool,         // --explain: verbose compilation steps
    lint_only: bool,       // --lint: only run linter
    bench: bool,           // --bench: benchmark compilation speed
    defines: Vec<(String, String)>,
    include_paths: Vec<String>,
}

fn parse_args() -> Args {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() { usage(); }

    let mut input = None;
    let mut output = None;
    let mut emit_asm = false;
    let mut compile_only = false;
    let mut preprocess_only = false;
    let mut dump_ast = false;
    let mut dump_ir = false;
    let mut use_ir = false;
    let mut emit_wasm = false;
    let mut explain = false;
    let mut lint_only = false;
    let mut bench = false;
    let mut defines = Vec::new();
    let mut include_paths = Vec::new();
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "-o" => { i += 1; if i >= args.len() { usage(); } output = Some(args[i].clone()); }
            "-S" => emit_asm = true,
            "-c" => compile_only = true,
            "-E" => preprocess_only = true,
            "--dump-ast" => dump_ast = true,
            "--ir" => { dump_ir = true; use_ir = true; }
            "--use-ir" => use_ir = true,
            "--wasm" => { emit_wasm = true; use_ir = true; }
            "--wasm-bin" => {
                // Compile directly to .wasm binary via IR
                use_ir = true;
                emit_wasm = false;
                // Handled specially after IR generation
            }
            "--explain" => explain = true,
            "--lint" => lint_only = true,
            "--bench" => { bench = true; }
            "--lsp" => { lsp::run_lsp(); process::exit(0); }
            "--help" | "-h" => {
                println!("rcc — a C compiler written in Rust");
                println!("\nUsage: rcc [ -o <path> ] [ -S ] [ -E ] [ -c ] <file>");
                println!("\nOptions:");
                println!("  -o <path>    Output file (default: <name>.exe)");
                println!("  -S           Emit assembly only (.s)");
                println!("  -c           Compile to object file only (.o)");
                println!("  -E           Preprocess only");
                println!("  -D name=val  Define preprocessor macro");
                println!("  -I path      Add include search path");
                println!("  --dump-ast   Dump AST");
                println!("  -h, --help   Show this help");
                process::exit(0);
            }
            arg if arg.starts_with("-D") => {
                let def = if arg.len() > 2 { arg[2..].to_string() } else { i += 1; args[i].clone() };
                if let Some(eq) = def.find('=') {
                    defines.push((def[..eq].to_string(), def[eq+1..].to_string()));
                } else {
                    defines.push((def, "1".to_string()));
                }
            }
            arg if arg.starts_with("-I") => {
                let path = if arg.len() > 2 { arg[2..].to_string() } else { i += 1; args[i].clone() };
                include_paths.push(path);
            }
            arg if arg.starts_with('-') => { eprintln!("error: unknown option: {}", arg); usage(); }
            _ => {
                if input.is_some() { usage(); }
                input = Some(args[i].clone());
            }
        }
        i += 1;
    }

    Args {
        input: input.unwrap_or_else(|| { eprintln!("error: no input file"); usage(); }),
        output, emit_asm, preprocess_only, compile_only, dump_ast,
        dump_ir, use_ir, emit_wasm, explain, lint_only, bench,
        defines, include_paths,
    }
}

fn read_file(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("\x1b[1;31merror\x1b[0m: cannot open '{}': {}", path, e);
        process::exit(1);
    })
}

fn run_bench(input: &str) {
    let source = read_file(input);
    let iterations = 100;

    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let mut pp = preprocess::Preprocessor::new();
        let processed = pp.preprocess(&source, input).unwrap_or_default();
        let tokens = lexer::tokenize(input, &processed).unwrap();
        let mut program = parser::Parser::new(tokens).parse_program().unwrap();
        optimize::optimize(&mut program);
        let _ = codegen::generate(&program);
    }
    let elapsed = start.elapsed();
    let per_iter = elapsed / iterations;

    eprintln!("\x1b[1;36mrcc benchmark\x1b[0m: {}", input);
    eprintln!("  {} iterations in {:.1?}", iterations, elapsed);
    eprintln!("  \x1b[1;32m{:.2?}/file\x1b[0m (preprocess + lex + parse + optimize + codegen)", per_iter);
    eprintln!("  source: {} lines, {} bytes", source.lines().count(), source.len());

    process::exit(0);
}

fn main() {
    let args = parse_args();
    if args.bench { run_bench(&args.input); }
    let raw_source = read_file(&args.input);

    // Preprocess
    let mut pp = preprocess::Preprocessor::new();
    for (name, val) in &args.defines { pp.defines_insert(name, val); }
    for path in &args.include_paths { pp.add_include_path(path); }

    let source = match pp.preprocess(&raw_source, &args.input) {
        Ok(s) => s,
        Err(e) => { eprintln!("\x1b[1;31merror\x1b[0m: {}", e); process::exit(1); }
    };

    if args.preprocess_only {
        if let Some(ref p) = args.output { std::fs::write(p, &source).unwrap(); }
        else { print!("{}", source); }
        return;
    }

    // Tokenize
    let tokens = match lexer::tokenize(&args.input, &source) {
        Ok(t) => t,
        Err(e) => { e.print(&args.input, &source); process::exit(1); }
    };

    // Parse
    let mut program = match parser::Parser::new(tokens).parse_program() {
        Ok(p) => p,
        Err(e) => { e.print(&args.input, &source); process::exit(1); }
    };

    // Optimize
    optimize::optimize(&mut program);

    if args.explain {
        eprintln!("\x1b[1;36m[1/5]\x1b[0m Preprocessing... {} lines", source.lines().count());
        eprintln!("\x1b[1;36m[2/5]\x1b[0m Tokenizing...");
        eprintln!("\x1b[1;36m[3/5]\x1b[0m Parsing... {} top-level declarations", program.decls.len());
        eprintln!("\x1b[1;36m[4/5]\x1b[0m Optimizing...");
    }

    // Run linter
    let warnings = lint::lint(&program);
    for w in &warnings {
        w.print(&args.input, &source);
    }
    if args.lint_only {
        if warnings.is_empty() {
            eprintln!("\x1b[1;32mNo warnings.\x1b[0m");
        } else {
            eprintln!("\n{} warning(s)", warnings.len());
        }
        return;
    }

    if args.dump_ast {
        for decl in &program.decls { println!("{:#?}", decl); }
        return;
    }

    // Generate assembly
    // Check for --wasm-bin (binary WASM output)
    if std::env::args().any(|a| a == "--wasm-bin") {
        let mut ir_module = lower::Lowering::new().lower(&program);
        for func in &mut ir_module.functions { ssa::promote_to_ssa(func); }
        ir_opt::optimize_ir(&mut ir_module);
        let wasm_bytes = wasm_encode::encode_wasm_from_ir(&ir_module);
        let out_path = args.output.clone().unwrap_or_else(|| {
            let stem = std::path::Path::new(&args.input).file_stem().unwrap().to_str().unwrap();
            format!("{}.wasm", stem)
        });
        std::fs::write(&out_path, &wasm_bytes).unwrap();
        eprintln!("\x1b[1;32mwrote\x1b[0m: {} ({} bytes)", out_path, wasm_bytes.len());
        return;
    }

    let asm = if args.emit_wasm {
        let mut ir_module = lower::Lowering::new().lower(&program);
        for func in &mut ir_module.functions { ssa::promote_to_ssa(func); }
        ir_opt::optimize_ir(&mut ir_module);
        let wat = backend_wasm::emit_wat(&ir_module);
        if let Some(ref p) = args.output {
            std::fs::write(p, &wat).unwrap();
            eprintln!("\x1b[1;32mwrote\x1b[0m: {}", p);
        } else {
            print!("{}", wat);
        }
        return;
    } else if args.use_ir {
        let mut ir_module = lower::Lowering::new().lower(&program);
        for func in &mut ir_module.functions { ssa::promote_to_ssa(func); }
        ir_opt::optimize_ir(&mut ir_module);
        if args.dump_ir {
            print!("{}", ir_module);
            return;
        }
        backend_x64::emit_x64(&ir_module)
    } else {
        // Legacy direct codegen
        match codegen::generate(&program) {
            Ok(a) => a,
            Err(e) => { e.print(&args.input, &source); process::exit(1); }
        }
    };

    // Apply peephole optimizations on assembly
    let asm = peephole::optimize_asm(&asm);

    // -S: emit assembly only
    if args.emit_asm {
        if let Some(ref p) = args.output { std::fs::write(p, &asm).unwrap(); }
        else { print!("{}", asm); }
        return;
    }

    // -c: compile to .o only
    if args.compile_only {
        let stem = std::path::Path::new(&args.input).file_stem().unwrap().to_str().unwrap();
        let asm_path = format!("{}.s", stem);
        let obj_path = args.output.clone().unwrap_or_else(|| format!("{}.o", stem));
        std::fs::write(&asm_path, &asm).unwrap();
        match driver::assemble(&asm_path, &obj_path) {
            Ok(()) => { let _ = std::fs::remove_file(&asm_path); }
            Err(e) => { eprintln!("\x1b[1;31merror\x1b[0m: {}", e); process::exit(1); }
        }
        return;
    }

    // Default: full compile → executable
    match driver::compile_to_exe(&asm, &args.input, args.output.as_deref()) {
        Ok(exe) => {
            eprintln!("\x1b[1;32mcompiled\x1b[0m: {}", exe);
        }
        Err(e) => {
            // Fallback: just output asm
            eprintln!("\x1b[1;33mwarning\x1b[0m: linking failed ({}), outputting assembly", e);
            let out = args.output.unwrap_or_else(|| {
                let stem = std::path::Path::new(&args.input).file_stem().unwrap().to_str().unwrap();
                format!("{}.s", stem)
            });
            std::fs::write(&out, &asm).unwrap();
            eprintln!("wrote {}", out);
        }
    }
}
