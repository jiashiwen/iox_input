use clap::{arg, Command};

pub fn new_task_cmd() -> Command {
    clap::Command::new("task")
        .about("command about task")
        .subcommand(cmd_task_create())
        .subcommand(cmd_task_start())
        .subcommand(cmd_task_stop())
        .subcommand(cmd_task_remove())
        .subcommand(cmd_task_list())
}

fn cmd_task_create() -> Command {
    clap::Command::new("create")
        .about("create task")
        .arg(arg!(<path> "create task json file path"))
}

fn cmd_task_start() -> Command {
    clap::Command::new("start")
        .about("start task")
        .arg(arg!(<taskid> "input task id to stop"))
}

fn cmd_task_stop() -> Command {
    clap::Command::new("stop")
        .about("stop task")
        .arg(arg!(<taskid>  "input task id to stop"))
}

fn cmd_task_remove() -> Command {
    clap::Command::new("remove")
        .about("remove task")
        .arg(arg!(<taskid>  "input task id to stop"))
}

fn cmd_task_list() -> Command {
    clap::Command::new("list")
        .about("list tasks")
        .subcommand(cmd_task_list_all())
        .subcommand(cmd_task_list_by_ids())
        .subcommand(cmd_task_list_by_names())
}

fn cmd_task_list_all() -> Command {
    clap::Command::new("all")
        .about("list tasks by task ids")
        .arg(arg!([queryid] "input queryid if have"))
}

fn cmd_task_list_by_ids() -> Command {
    clap::Command::new("byid")
        .about("list tasks by task ids")
        .arg(arg!(<taskid> "input taskid"))
}

fn cmd_task_list_by_names() -> Command {
    clap::Command::new("bynames")
        .about("list tasks by task names")
        .arg(arg!(<tasksname>
            r"input tasks name if multi use ',' to splite"
        ))
}
