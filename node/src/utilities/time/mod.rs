use std::time::Duration;

pub fn duration_now() -> Duration {
    use std::time::SystemTime;
    let now = SystemTime::now();
    now.duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_else(|e| {
            panic!("Current time {now:?} is before unix epoch. Something is wrong: {e:?}")
        })
}
