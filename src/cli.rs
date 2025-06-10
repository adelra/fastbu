use clap::{App, Arg};

fn main() {
    let matches = App::new("Fastbu CLI")
        .arg(Arg::with_name("host").short("h").long("host").default_value("127.0.0.1"))
        .arg(Arg::with_name("port").short("p").long("port").default_value("3030"))
        .subcommand(SubCommand::with_name("serve"))
        .subcommand(SubCommand::with_name("set").about("Set a key-value pair"))
        .get_matches();

    // Handle subcommands
    if let Some(_) = matches.subcommand_matches("serve") {
        start_server(matches.value_of("host"), matches.value_of("port"));
    }
}