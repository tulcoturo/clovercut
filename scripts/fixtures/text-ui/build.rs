// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

fn main() {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(|| {
            let config = slint_build::CompilerConfiguration::new().with_debug_info(true);
            slint_build::compile_with_config("probe.slint", config).unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}
