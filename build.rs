use std::env;

fn main() {
    let target = env::var("TARGET").unwrap();
    if target.contains("windows") {
        cc::Build::new()
            .file("src/arch/windows.c")
            .compile("stackful");
        return;
    }

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let file = match &*target_arch {
        "x86_64" => "src/arch/x86_64.S",
        "x86" => "src/arch/x86.s",
        "aarch64" => "src/arch/aarch64.S",
        "riscv64" => "src/arch/riscv64.s",
        "wasm32" => "src/arch/wasm32.s",
        _ => {
            panic!("Current architecture {} is not supported", target_arch);
        }
    };
    cc::Build::new().file(file).compile("stackful");
}
