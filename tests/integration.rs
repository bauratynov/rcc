use std::process::Command;
use std::path::Path;

fn compile_test(input: &str) -> bool {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let bin = Path::new(manifest_dir).join("target/debug/rcc.exe");
    if !bin.exists() { return false; }

    let input_path = Path::new(manifest_dir).join(input);
    let output = Command::new(&bin)
        .args([input_path.to_str().unwrap()])
        .output()
        .expect("failed to run rcc");
    output.status.success()
}

macro_rules! test_case {
    ($name:ident, $file:expr) => {
        #[test]
        fn $name() { assert!(compile_test($file)); }
    };
    ($name:ident, $file:expr, should_fail) => {
        #[test]
        fn $name() { assert!(!compile_test($file)); }
    };
}

test_case!(test_hello, "test_inputs/hello.c");
test_case!(test_simple, "test_inputs/simple.c");
test_case!(test_loops, "test_inputs/loops.c");
test_case!(test_operators, "test_inputs/operators.c");
test_case!(test_pointers, "test_inputs/pointers.c");
test_case!(test_switch, "test_inputs/switch.c");
test_case!(test_ternary, "test_inputs/ternary.c");
test_case!(test_fibonacci, "test_inputs/fibonacci.c");
test_case!(test_compound_assign, "test_inputs/compound_assign.c");
test_case!(test_nested_calls, "test_inputs/nested_calls.c");
test_case!(test_global_var, "test_inputs/global_var.c");
test_case!(test_optimize, "test_inputs/optimize.c");
test_case!(test_preprocess, "test_inputs/preprocess2.c");
test_case!(test_bad_file_fails, "test_inputs/bad.c", should_fail);
