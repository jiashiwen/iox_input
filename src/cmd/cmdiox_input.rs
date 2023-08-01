use clap::{Arg, Command};

pub fn new_iox_input_cmd() -> Command {
    clap::Command::new("iox_input")
        .about("iox_input")
        .args(&[Arg::new("server")
            .value_name("server")
            .required(true)
            .index(1)
            .help("fluxdb-iox server address like 'http://127.0.0.1:8080'")])
        .args(&[Arg::new("namespace")
            .value_name("namespace")
            .required(true)
            .index(2)
            .help("namespace name to input into")])
        .args(&[Arg::new("file")
            .value_name("file")
            .required(true)
            .index(3)
            .help("input file")])
}
