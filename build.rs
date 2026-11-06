use std::{env, fs::File, io::Write, path::PathBuf};

const REV_A: &str = "rev-a";
const REV_B: &str = "rev-b";

const REV_A_ORIGIN: &str = "0x08000000";
const REV_B_ORIGIN: &str = "0x08002000";
const REV_B_BOOTLOADER_SIZE: &str = "0x2000";
const RGB_STORE_SIZE: &str = "4K";

const LAYOUT: &str = "
MEMORY
{
    FLASH : ORIGIN = {ORG}, LENGTH = 128K {EXT}
    RAM   : ORIGIN = 0x20000000, LENGTH = 32K
    /* CCMRAM : ORIGIN = 0x10000000, LENGTH = 8K */
}
";

fn main() {
    println!("cargo:rerun-if-env-changed=REVISION");

    let revision = env::var("KEYBOARD_REVISION")
        .expect("\nKEYBOARD_REVISION must be set ('rev-a' or 'rev-b')");
    assert!(
        revision == REV_A || revision == REV_B,
        "\nRevision '{revision}' not supported"
    );

    let mem_x = format_mem_layout(&revision);
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(mem_x.as_bytes())
        .unwrap();

    #[cfg(feature = "defmt")]
    println!("cargo:rustc-link-arg=-Tdefmt.x");
    println!("cargo:rustc-link-search={}", out.display());
}

fn format_mem_layout(revision: &str) -> String {
    let origin = match revision {
        REV_A => REV_A_ORIGIN,
        REV_B => REV_B_ORIGIN,
        _ => unreachable!(),
    };

    LAYOUT.replace("{ORG}", origin).replace(
        "{EXT}",
        &match (matches!(revision, REV_B), cfg!(feature = "rgb")) {
            (true, true) => format!("- {REV_B_BOOTLOADER_SIZE} - {RGB_STORE_SIZE}"),
            (true, false) => format!("- {REV_B_BOOTLOADER_SIZE}"),
            (false, true) => format!("- {RGB_STORE_SIZE}"),
            (false, false) => String::default(),
        },
    )
}
