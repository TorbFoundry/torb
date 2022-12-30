use clap::{AppSettings, Arg, Command, SubCommand};

pub fn cli() -> Command<'static> {
    Command::new("torb")
        .version("1.0.0")
        .author("Torb Foundry")
        .setting(AppSettings::ArgRequiredElseHelp)
        .subcommand(SubCommand::with_name("version").about("Get the version of this torb."))
        .subcommand(
            SubCommand::with_name("init").about("Initialize Torb, download artifacts and tools."),
        )
        .subcommand(
            SubCommand::with_name("repo")
                .about("Verbs for interacting with project repos.")
                .setting(AppSettings::ArgRequiredElseHelp)
                .subcommand(
                    SubCommand::with_name("create")
                        .about("Create a new repository for a Torb stack.")
                        .arg(
                            Arg::with_name("path")
                                .takes_value(true)
                                .required(true)
                                .index(1)
                                .help("Path of the repo to create."),
                        )
                        .arg(
                            Arg::new("--local-only")
                                .short('l')
                                .required(false)
                                .takes_value(false)
                                .help("Only create the repo locally."),
                        ),
                ),
        )
        .subcommand(
            SubCommand::with_name("stack")
                .about("Verbs for interacting with Torb stacks.")
                .setting(AppSettings::ArgRequiredElseHelp)
                .subcommand(
                    SubCommand::with_name("checkout")
                        .about("Add a stack definition template to your current directory.")
                        .arg(
                            Arg::with_name("name")
                                .takes_value(true)
                                .required(false)
                                .index(1)
                                .help("Name of the stack definition template to checkout."),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("init")
                        .about("Run any init steps for a stack's dependencies.")
                        .arg(
                            Arg::with_name("file")
                                .takes_value(true)
                                .required(true)
                                .index(1)
                                .help("File path of the stack definition file."),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("build")
                        .about("Build a stack from a stack definition file.")
                        .arg(
                            Arg::with_name("file")
                                .takes_value(true)
                                .required(true)
                                .index(1)
                                .help("File path of the stack definition file."),
                        )
                        .arg(
                            Arg::new("--dryrun")
                                .short('d')
                                .takes_value(false)
                                .help("Dry run. Don't actually build the stack."),
                        )
                        .arg(
                            Arg::new("--platforms")
                                .short('p')
                                .default_values(&["linux/amd64", "linux/arm64"])
                                .help(
                                    "Comma separated list of platforms to build docker images for.",
                                ),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("deploy")
                        .about("Deploy a stack from a stack definition file.")
                        .arg(
                            Arg::with_name("file")
                                .takes_value(true)
                                .required(true)
                                .index(1)
                                .help("File path of the stack definition file."),
                        )
                        .arg(
                            Arg::new("--dryrun")
                                .short('d')
                                .takes_value(false)
                                .help("Dry run. Don't actually deploy the stack."),
                        ),
                )
                .subcommand(SubCommand::with_name("list").about("List all available stacks.")),
        )
}
