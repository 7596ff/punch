use directories::BaseDirs;
use std::fs::DirBuilder;
use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::io::Write;
use std::path::PathBuf;
use std::process;

pub fn get_conf_file(read: bool, append: bool) -> io::Result<File> {
    let mut conf_file = PathBuf::new();
    conf_file.push(BaseDirs::new().unwrap().home_dir());
    conf_file.push(".punch");
    conf_file.push("punch.log");

    OpenOptions::new().read(read).append(append).open(conf_file)
}

pub fn append_to_file(data: &[u8], f: &mut File) {
    match f.write_all(data) {
        Ok(_) => {}
        Err(e) => println!("Failed to write data to log: {}", e),
    }
}

pub fn ensure_log_file_exists() -> io::Result<()> {
    let mut conf_dir = PathBuf::new();
    conf_dir.push(BaseDirs::new().unwrap().home_dir());
    conf_dir.push(".punch");
    let config_path = conf_dir.as_path();

    let mut conf_file_builder = PathBuf::from(config_path);
    conf_file_builder.push("punch.log");

    let mut dir_builder = DirBuilder::new();
    dir_builder.recursive(true);

    dir_builder.create(config_path)?;

    let conf_file = conf_file_builder.as_path();
    match OpenOptions::new().create(true).write(true).open(conf_file) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

pub fn exit_if_log_file_cannot_be_created() {
    match ensure_log_file_exists() {
        Ok(_) => {}
        Err(e) => {
            println!("Couldn't create punch log: {}.\nExiting.", e);
            process::exit(1)
        }
    }
}
