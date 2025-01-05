use env_logger::{Builder, Target};

pub fn init() {
    let env = env_logger::Env::default().filter_or("RUST_LOG", "info");

    Builder::from_env(env)
        .target(Target::Stdout)
        .format_timestamp(None)
        .format_module_path(false)
        .init();
}
