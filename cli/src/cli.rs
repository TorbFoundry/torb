// Business Source License 1.1
// Licensor:  Torb Foundry
// Licensed Work:  Torb v0.3.5-03.13
// The Licensed Work is © 2023-Present Torb Foundry
//
// Change License: GNU Affero General Public License Version 3
// Additional Use Grant: None
// Change Date: Feb 22, 2023
//
// See LICENSE file at https://github.com/TorbFoundry/torb/blob/main/LICENSE for details.

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
            SubCommand::with_name("artifacts")
            .about("Verbs for interacting with artifact repositories.")
            .setting(AppSettings::ArgRequiredElseHelp)
            .subcommand(
                SubCommand::with_name("clone")
                    .about("Iterate through `repositories` config option and clone all that don't exist.")
            )
            .subcommand(
                SubCommand::with_name("refresh")
                    .about("Iterate through the .torb/repositories entries and pull --rebase to latest commit. Can be configured to act on specific repos, see help for details.")
                    .arg(
                        Arg::new("name")
                            .long("name")
                            .takes_value(true)
                            .required(false)
                            .default_value("")
                            .short('n')
                    )
            )
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
                    SubCommand::with_name("new")
                        .about("Create a new stack.yaml template.")
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
                                .long("dryrun")
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
                        )
                        .arg(
                            Arg::new("--local-hosted-registry")
                                .short('l')
                                .long("local-hosted-registry")
                                .takes_value(false)
                                .help("Runs the builder with the docker driver to push to a separate registry hosted on localhost (or an address pointing to localhost)"),
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
                                .long("dryrun")
                                .takes_value(false)
                                .help("Dry run. Don't actually deploy the stack."),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("watch")
                        .about("Watch files for changes and re-build and redeploy to cluster.")
                        .arg(
                            Arg::with_name("file")
                                .takes_value(true)
                                .required(true)
                                .index(1)
                                .help("File path of the stack definition file."),
                        )
                        .arg(
                            Arg::new("--local-hosted-registry")
                                .short('l')
                                .long("local-hosted-registry")
                                .takes_value(false)
                                .help("Runs the builder with the docker driver to push to a separate registry hosted on localhost (or an address pointing to localhost)"),
                        ),
                )
                .subcommand(SubCommand::with_name("list").about("List all available stacks.")),
        )
}
