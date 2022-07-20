struct ResolverConfig {
    autoaccept: bool,
    stack_path: String,
    stack_name: String,
    stack_description: String,
    stack_contents: String
}

impl ResolverConfig {
    fn new(autoaccept: bool, stack_path: String, stack_name: String, stack_description: String, stack_contents: String) -> ResolverConfig {
        ResolverConfig {
            autoaccept,
            stack_path,
            stack_name,
            stack_description,
            stack_contents
        }
    }
}

struct Resolver {
    config: &ResolverConfig,
}

impl Resolver {
    pub fn new(config: &ResolverConfig) -> Resolver {
        Resolver {
            config: config,
        }
    }

}