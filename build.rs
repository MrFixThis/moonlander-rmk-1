#[cfg(any(
    not(any(feature = "rev_a", feature = "rev_b")),
    all(feature = "rev_a", feature = "rev_b")
))]
compile_error!(
    "A keyboard revision must be specified (select either 'rev_a' or 'rev_b', not both)!"
);

#[cfg(feature = "vial")]
use std::path::Path;

use std::{
    env,
    fs::{self, File},
    io::Write,
    path::PathBuf,
};

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let mem_file = memory_file();
    let mem_file_bytes =
        fs::read(mem_file).unwrap_or_else(|err| panic!("Could not read {mem_file}: {err}"));
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(&mem_file_bytes)
        .unwrap();

    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rustc-link-arg=--nmagic");
    println!("cargo:rustc-link-arg=-Tlink.x");
    println!("cargo:rustc-linker=flip-link");

    println!("cargo:rerun-if-changed={mem_file}");

    #[cfg(feature = "vial")]
    generate_vial_config(&out);
}

#[allow(unused)]
#[inline(always)]
const fn memory_file() -> &'static str {
    #[cfg(feature = "rev_a")]
    let mem_file = "memory/reva.x";
    #[cfg(feature = "rev_b")]
    let mem_file = "memory/revb.x";
    mem_file
}

#[cfg(feature = "vial")]
fn generate_vial_config(out: &Path) {
    use std::{fs, io::Read};

    use const_gen::*;
    use xz2::read::XzEncoder;

    let vial_config_file = env::var_os("VIAL_CONFIG_PATH").unwrap_or("vial.json".into());
    let vial_config_path = Path::new(&vial_config_file);
    let content = fs::read_to_string(vial_config_path).unwrap_or_else(|err| {
        panic!(
            "Could not read vial.json at {}: {err}",
            vial_config_path.display()
        )
    });

    let vial_config = json::stringify(json::parse(&content).expect("Invalid vial.json config"));
    let mut keyboard_def_compressed: Vec<u8> = Vec::new();
    XzEncoder::new(vial_config.as_bytes(), 6)
        .read_to_end(&mut keyboard_def_compressed)
        .unwrap();

    let keyboard_id: Vec<u8> = vec![0x4D, 0x4C, 0x5F, 0x52, 0x4D, 0x4B, 0x5F, 0x49]; // ML_RMK_I
    let const_declarations = [
        const_declaration!(pub VIAL_KEYBOARD_DEF = keyboard_def_compressed),
        const_declaration!(pub VIAL_KEYBOARD_ID = keyboard_id),
    ]
    .map(|s| "#[allow(clippy::redundant_static_lifetimes)]\n".to_owned() + s.as_str())
    .join("\n");

    fs::write(out.join("vial_gen_config.rs"), const_declarations).unwrap();

    println!("cargo:rerun-if-changed={}", vial_config_path.display());
}
