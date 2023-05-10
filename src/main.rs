use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env!("OUT_DIR"));
    let nukleus = PathBuf::from(env!("CARGO_BIN_FILE_NUKLEUS_nukleus"));
    let initium = PathBuf::from(env!("CARGO_BIN_FILE_INITIUM_initium"));

}