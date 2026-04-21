use std::process::Command;
use std::path::Path;

fn find_cc() -> Option<String> {
    #[cfg(target_os = "windows")]
    let candidates = &["C:/mingw64/bin/gcc.exe", "gcc.exe", "gcc"];
    #[cfg(not(target_os = "windows"))]
    let candidates = &["cc", "gcc"];

    for cc in candidates {
        if let Ok(out) = Command::new(cc).arg("--version").output() {
            if out.status.success() { return Some(cc.to_string()); }
        }
    }
    None
}

fn default_exe_name(stem: &str) -> String {
    #[cfg(target_os = "windows")]
    { format!("{}.exe", stem) }
    #[cfg(not(target_os = "windows"))]
    { stem.to_string() }
}

pub fn assemble_and_link(asm_path: &str, output_path: &str) -> Result<(), String> {
    let cc = find_cc().ok_or("C compiler not found. Install gcc or cc.")?;

    let mut args = vec!["-o", output_path, asm_path];

    #[cfg(target_os = "windows")]
    args.push("-static");

    #[cfg(target_os = "linux")]
    { args.push("-lm"); }

    let status = Command::new(&cc)
        .args(&args)
        .status()
        .map_err(|e| format!("cc failed: {}", e))?;

    if !status.success() {
        return Err("linking failed".to_string());
    }
    Ok(())
}

pub fn assemble(asm_path: &str, obj_path: &str) -> Result<(), String> {
    let cc = find_cc().ok_or("C compiler not found")?;
    let status = Command::new(&cc)
        .args(["-c", asm_path, "-o", obj_path])
        .status()
        .map_err(|e| format!("cc failed: {}", e))?;
    if !status.success() { return Err("assembly failed".to_string()); }
    Ok(())
}

pub fn compile_to_exe(asm: &str, input_path: &str, output_path: Option<&str>) -> Result<String, String> {
    let input = Path::new(input_path);
    let stem = input.file_stem().unwrap().to_str().unwrap();
    let dir = input.parent().map(|p| p.to_str().unwrap()).unwrap_or(".");

    let asm_path = format!("{}/{}.s", dir, stem);
    let exe_path = output_path
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}/{}", dir, default_exe_name(stem)));

    std::fs::write(&asm_path, asm)
        .map_err(|e| format!("cannot write {}: {}", asm_path, e))?;

    assemble_and_link(&asm_path, &exe_path)?;
    let _ = std::fs::remove_file(&asm_path);
    Ok(exe_path)
}
