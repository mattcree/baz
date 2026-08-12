//! Embed the same red-circle mark that the running app and hicolor theme use
//! into native Windows executables.

#[cfg(windows)]
fn main() {
    println!("cargo:rerun-if-changed=assets/icons/logo-transparent-circle-red.ico");
    winres::WindowsResource::new()
        .set_icon("assets/icons/logo-transparent-circle-red.ico")
        .compile()
        .expect("embed baz's red-circle Windows executable icon");
}

#[cfg(not(windows))]
fn main() {
    println!("cargo:rerun-if-changed=assets/icons/logo-transparent-circle-red.ico");
}
