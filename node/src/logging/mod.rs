use lazy_static::lazy_static;
use serde::Serialize;

lazy_static! {
    pub static ref RUST_LOG_JSON: bool = std::env::var("RUST_LOG_JSON").is_ok();
}

pub fn init() {
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

pub fn init_with_directives(directives: &str) {
    println!("Logging enabled with directives: {directives}",);
    pretty_env_logger::formatted_timed_builder()
        .parse_filters(directives)
        .format_timestamp_millis()
        .init();
}

pub fn pretty_json<T: Serialize + std::fmt::Debug>(value: &T) -> String {
    if *RUST_LOG_JSON {
        let json = serde_json::json!(&value);
        match serde_json::to_string_pretty(&json) {
            Ok(s) => s,
            _ => json.to_string(),
        }
    } else {
        format!("{value:?}",)
    }
}
