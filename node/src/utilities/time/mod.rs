use chrono::Utc;

pub fn ephemera_now() -> u64 {
    Utc::now().timestamp_millis() as u64
}
