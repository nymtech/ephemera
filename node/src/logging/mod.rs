#![allow(dead_code)]
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

pub fn init_logging_with_directives(directives: &str) {
    println!("Logging enabled with directives: {directives}",);
    pretty_env_logger::formatted_timed_builder()
        .parse_filters(directives)
        .format_timestamp_millis()
        .init();
}
