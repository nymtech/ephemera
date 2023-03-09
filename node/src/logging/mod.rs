//! Logging setup

pub fn init_logging() {
    if let Ok(directives) = ::std::env::var("RUST_LOG") {
        println!("Logging enabled with directives: {directives}",);
        pretty_env_logger::formatted_timed_builder()
            .parse_filters(&directives)
            .format_timestamp_millis()
            .init();
    } else {
        println!("Logging disabled");
    }
}
