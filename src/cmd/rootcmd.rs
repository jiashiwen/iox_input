use crate::cmd::{new_iox_input_cmd, new_use_log_cmd};
use crate::commons::{format_to_lp, write_json_to_iox, SubCmd};
use crate::commons::{gen_iox_client, CommandCompleter};
use crate::configure::get_config;

use crate::interact::{self, INTERACT_STATUS};
use crate::request::Request;

use clap::{Arg, ArgAction, ArgMatches, Command as clap_Command};

use futures::stream;
use lazy_static::lazy_static;
use std::borrow::Borrow;
use std::f32::consts::E;
use std::fs::File;
use std::io::{BufRead, BufReader};
use tokio::runtime;
use tokio::task::JoinSet;

use sysinfo::{PidExt, System, SystemExt};

lazy_static! {
    static ref CLIAPP: clap::Command = clap::Command::new("iox_input")
        .version("1.0")
        .author("Shiwen Jia. <jiashiwen@gmail.com>")
        .about("command line sample")
        .arg_required_else_help(true)
        .arg(
            Arg::new("interact")
                .short('i')
                .long("interact")
                // .conflicts_with("daemon")
                .action(ArgAction::SetTrue)
                .help("run as interact mod")
        )
        .subcommand(new_iox_input_cmd())
        .subcommand(new_use_log_cmd());
    static ref SUBCMDS: Vec<SubCmd> = subcommands();
}

pub fn run_app() {
    let matches = CLIAPP.clone().get_matches();
    // if let Some(c) = matches.get_one::<String>("config") {
    //     println!("config path is:{}", c);
    //     set_config_file_path(c.to_string());
    // }
    cmd_match(&matches);
}

pub fn run_from(args: Vec<String>) {
    match clap_Command::try_get_matches_from(CLIAPP.to_owned(), args.clone()) {
        Ok(matches) => {
            cmd_match(&matches);
        }
        Err(err) => {
            err.print().expect("Error writing Error");
        }
    };
}

// 获取全部子命令，用于构建commandcompleter
pub fn all_subcommand(app: &clap_Command, beginlevel: usize, input: &mut Vec<SubCmd>) {
    let nextlevel = beginlevel + 1;
    let mut subcmds = vec![];
    for iterm in app.get_subcommands() {
        subcmds.push(iterm.get_name().to_string());
        if iterm.has_subcommands() {
            all_subcommand(iterm, nextlevel, input);
        } else {
            if beginlevel == 0 {
                all_subcommand(iterm, nextlevel, input);
            }
        }
    }
    let subcommand = SubCmd {
        level: beginlevel,
        command_name: app.get_name().to_string(),
        subcommands: subcmds,
    };
    input.push(subcommand);
}

pub fn get_command_completer() -> CommandCompleter {
    CommandCompleter::new(SUBCMDS.to_vec())
}

fn subcommands() -> Vec<SubCmd> {
    let mut subcmds = vec![];
    all_subcommand(CLIAPP.clone().borrow(), 0, &mut subcmds);
    subcmds
}

pub fn process_exists(pid: &u32) -> bool {
    let mut sys = System::new_all();
    sys.refresh_all();
    for (syspid, _) in sys.processes() {
        if syspid.as_u32().eq(pid) {
            return true;
        }
    }
    return false;
}

fn cmd_match(matches: &ArgMatches) {
    // if let Some(c) = matches.get_one::<String>("config") {
    //     // if let Some(c) = matches.value_of("config") {
    //     set_config_file_path(c.to_string());
    //     set_config_from_file(&get_config_file_path());
    // } else {
    //     set_config_from_file("");
    // }
    let config = get_config().unwrap();
    let server = config.server;
    let req = Request::new(server.clone());

    // if matches.get_flag("daemon") {
    //     // if matches.is_present("daemon") {
    //     let args: Vec<String> = env::args().collect();
    //     if let Ok(Fork::Child) = daemon(true, true) {
    //         // 启动子进程
    //         let mut cmd = Command::new(&args[0]);

    //         for idx in 1..args.len() {
    //             let arg = args.get(idx).expect("get cmd arg error!");
    //             // 去除后台启动参数,避免重复启动
    //             if arg.eq("-d") || arg.eq("-daemon") {
    //                 continue;
    //             }
    //             cmd.arg(arg);
    //         }

    //         let child = cmd.spawn().expect("Child process failed to start.");
    //         fs::write("pid", child.id().to_string()).unwrap();
    //         println!("process id is:{}", std::process::id());
    //         println!("child id is:{}", child.id());
    //     }
    //     println!("{}", "daemon mod");
    //     process::exit(0);
    // }

    if matches.get_flag("interact") {
        if !INTERACT_STATUS.load(std::sync::atomic::Ordering::SeqCst) {
            interact::run();
            return;
        }
    }

    // 测试 log 写入不同文件
    if let Some(ref log) = matches.subcommand_matches("uselog") {
        println!("use log");
        if let Some(_) = log.subcommand_matches("syslog") {
            log::info!(target:"syslog","Input sys log");
        }

        if let Some(_) = log.subcommand_matches("businesslog") {
            log::info!(target:"businesslog","Input business log");
        }
    }

    let Some(ref iox_input) = matches.subcommand_matches("iox_input") else { return };
    let server = match iox_input.get_one::<String>("server") {
        Some(s) => s,
        None => {
            return;
        }
    };
    let namespace = match iox_input.get_one::<String>("namespace") {
        Some(s) => s,
        None => {
            return;
        }
    };
    let file_path = match iox_input.get_one::<String>("file") {
        Some(s) => s,
        None => {
            return;
        }
    };
    let task_banch = 200;
    let max_tasks = 16;
    let rt = runtime::Builder::new_multi_thread()
        .worker_threads(num_cpus::get())
        .enable_all()
        // .max_io_events_per_tick(self.task_threads)
        .build()
        .unwrap();
    let mut set: JoinSet<()> = JoinSet::new();
    rt.block_on(async {
        // let mut iox_client = gen_iox_client(server).await.unwrap();
        let file = File::open(file_path).unwrap();

        let mut vec_json_str: Vec<String> = vec![];

        // 按行读文件
        let lines = BufReader::new(file).lines();
        for line in lines {
            match line {
                Ok(s) => {
                    vec_json_str.push(s);
                }
                Err(e) => {
                    log::error!("{}", e);
                    continue;
                }
            };

            if vec_json_str.len().eq(&task_banch) {
                while set.len() >= max_tasks {
                    set.join_next().await;
                }
                let vj = vec_json_str.clone();
                let server = server.clone();
                let ns: String = namespace.clone();
                set.spawn(async move {
                    if let Err(e) = write_json_to_iox(&server, ns.as_str(), vj).await {
                        log::error!("{}", e);
                    };
                });
                vec_json_str.clear();
            }
        }

        if vec_json_str.len() > 0 {
            while set.len() >= max_tasks {
                set.join_next().await;
            }
            let vj = vec_json_str.clone();
            let server = server.clone();
            let ns: String = namespace.clone();
            set.spawn(async move {
                if let Err(e) = write_json_to_iox(&server, ns.as_str(), vj).await {
                    log::error!("{}", e);
                };
            });
        }

        while set.len() > 0 {
            set.join_next().await;
        }
    });
}
