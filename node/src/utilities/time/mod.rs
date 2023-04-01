use chrono::Utc;

pub struct EphemeraTime;

impl EphemeraTime {
    pub fn now() -> u64 {
        Utc::now().timestamp_millis() as u64
    }
}
