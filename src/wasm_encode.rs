/// WASM binary encoder
///
/// Encodes WAT-like structures directly to .wasm binary format.
/// Produces valid WASM modules without external tools.
///
/// WASM binary format: https://webassembly.github.io/spec/core/binary/

pub fn encode_minimal_wasm(main_returns: i64) -> Vec<u8> {
    let mut wasm = Vec::new();

    // Magic number + version
    wasm.extend_from_slice(b"\0asm");
    wasm.extend_from_slice(&1u32.to_le_bytes());

    // Type section (section 1): one function type () -> i32
    let type_section = {
        let mut s = Vec::new();
        s.push(1); // 1 type
        s.push(0x60); // func type
        s.push(0); // 0 params
        s.push(1); // 1 result
        s.push(0x7F); // i32
        s
    };
    write_section(&mut wasm, 1, &type_section);

    // Function section (section 3): one function using type 0
    let func_section = {
        let mut s = Vec::new();
        s.push(1); // 1 function
        s.push(0); // type index 0
        s
    };
    write_section(&mut wasm, 3, &func_section);

    // Export section (section 7): export "main" as function 0
    let export_section = {
        let mut s = Vec::new();
        s.push(1); // 1 export
        write_string(&mut s, "main");
        s.push(0x00); // func export
        s.push(0); // func index 0
        s
    };
    write_section(&mut wasm, 7, &export_section);

    // Code section (section 10): function body
    let code_section = {
        let mut s = Vec::new();
        s.push(1); // 1 function body

        let body = {
            let mut b = Vec::new();
            b.push(0); // 0 locals

            // i32.const <value>
            b.push(0x41);
            write_sleb128(&mut b, main_returns as i32);

            // end
            b.push(0x0B);
            b
        };

        write_u32_leb128(&mut s, body.len() as u32);
        s.extend_from_slice(&body);
        s
    };
    write_section(&mut wasm, 10, &code_section);

    wasm
}

/// Encode a full WASM module from IR
pub fn encode_wasm_from_ir(module: &crate::ir::IrModule) -> Vec<u8> {
    let mut wasm = Vec::new();

    // Magic + version
    wasm.extend_from_slice(b"\0asm");
    wasm.extend_from_slice(&1u32.to_le_bytes());

    // Count functions
    let funcs: Vec<&crate::ir::IrFunction> = module.functions.iter().collect();
    let nfuncs = funcs.len();

    // Type section: one type per function
    let type_section = {
        let mut s = Vec::new();
        write_u32_leb128(&mut s, nfuncs as u32);
        for func in &funcs {
            s.push(0x60); // func type
            write_u32_leb128(&mut s, func.params.len() as u32);
            for _ in &func.params {
                s.push(0x7E); // i64
            }
            if func.return_ty == crate::ir::IrType::Void {
                s.push(0); // 0 results
            } else {
                s.push(1); // 1 result
                s.push(0x7E); // i64
            }
        }
        s
    };
    write_section(&mut wasm, 1, &type_section);

    // Function section
    let func_section = {
        let mut s = Vec::new();
        write_u32_leb128(&mut s, nfuncs as u32);
        for (i, _) in funcs.iter().enumerate() {
            write_u32_leb128(&mut s, i as u32); // type index
        }
        s
    };
    write_section(&mut wasm, 3, &func_section);

    // Memory section: 1 page
    let mem_section = {
        let mut s = Vec::new();
        s.push(1); // 1 memory
        s.push(0x00); // no max
        s.push(2); // 2 pages initial
        s
    };
    write_section(&mut wasm, 5, &mem_section);

    // Export section: export "main" and "memory"
    let export_section = {
        let mut s = Vec::new();
        let mut nexports = 1u32; // memory
        for func in &funcs {
            if func.name == "main" { nexports += 1; }
        }
        write_u32_leb128(&mut s, nexports);
        // Export memory
        write_string(&mut s, "memory");
        s.push(0x02); // memory export
        s.push(0); // memory index

        // Export main
        for (i, func) in funcs.iter().enumerate() {
            if func.name == "main" {
                write_string(&mut s, "main");
                s.push(0x00); // func export
                write_u32_leb128(&mut s, i as u32);
            }
        }
        s
    };
    write_section(&mut wasm, 7, &export_section);

    // Data section: string literals
    if !module.data.is_empty() {
        let data_section = {
            let mut s = Vec::new();
            write_u32_leb128(&mut s, module.data.len() as u32);
            let mut offset = 0u32;
            for d in &module.data {
                s.push(0x00); // active, memory 0
                // i32.const offset
                s.push(0x41);
                write_sleb128(&mut s, offset as i32);
                s.push(0x0B); // end
                write_u32_leb128(&mut s, d.bytes.len() as u32);
                s.extend_from_slice(&d.bytes);
                offset += d.bytes.len() as u32;
            }
            s
        };
        write_section(&mut wasm, 11, &data_section);
    }

    // Code section: function bodies (simplified — just return 0 for now)
    let code_section = {
        let mut s = Vec::new();
        write_u32_leb128(&mut s, nfuncs as u32);
        for func in &funcs {
            let body = {
                let mut b = Vec::new();
                // Locals
                let nlocals = func.next_vreg as u32;
                if nlocals > 0 {
                    b.push(1); // 1 local decl
                    write_u32_leb128(&mut b, nlocals);
                    b.push(0x7E); // i64
                } else {
                    b.push(0); // 0 local decls
                }

                // Body: just return 0 for non-trivial functions
                if func.return_ty != crate::ir::IrType::Void {
                    b.push(0x42); // i64.const
                    write_sleb128_64(&mut b, 0);
                }
                b.push(0x0B); // end
                b
            };
            write_u32_leb128(&mut s, body.len() as u32);
            s.extend_from_slice(&body);
        }
        s
    };
    write_section(&mut wasm, 10, &code_section);

    wasm
}

// ── Helpers ──

fn write_section(out: &mut Vec<u8>, id: u8, data: &[u8]) {
    out.push(id);
    write_u32_leb128(out, data.len() as u32);
    out.extend_from_slice(data);
}

fn write_string(out: &mut Vec<u8>, s: &str) {
    write_u32_leb128(out, s.len() as u32);
    out.extend_from_slice(s.as_bytes());
}

fn write_u32_leb128(out: &mut Vec<u8>, mut val: u32) {
    loop {
        let mut byte = (val & 0x7F) as u8;
        val >>= 7;
        if val != 0 { byte |= 0x80; }
        out.push(byte);
        if val == 0 { break; }
    }
}

fn write_sleb128(out: &mut Vec<u8>, mut val: i32) {
    loop {
        let mut byte = (val & 0x7F) as u8;
        val >>= 7;
        let more = !(((val == 0) && (byte & 0x40 == 0)) || ((val == -1) && (byte & 0x40 != 0)));
        if more { byte |= 0x80; }
        out.push(byte);
        if !more { break; }
    }
}

fn write_sleb128_64(out: &mut Vec<u8>, mut val: i64) {
    loop {
        let mut byte = (val & 0x7F) as u8;
        val >>= 7;
        let more = !(((val == 0) && (byte & 0x40 == 0)) || ((val == -1) && (byte & 0x40 != 0)));
        if more { byte |= 0x80; }
        out.push(byte);
        if !more { break; }
    }
}
