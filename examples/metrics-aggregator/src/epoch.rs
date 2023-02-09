
use std::ops::Mul;



use time::{Duration, OffsetDateTime};


#[derive(Debug, Clone, Copy)]
pub(crate) struct Epoch {
    pub(crate) start_time: OffsetDateTime,
    pub(crate) duration: Duration,
}

impl Epoch {
    pub(crate) fn new(start_time: OffsetDateTime, duration: Duration) -> Self {
        Self {
            start_time,
            duration,
        }
    }

    pub(crate) fn current_epoch_start_time(&self) -> OffsetDateTime {
        let since_start =
            OffsetDateTime::now_utc().unix_timestamp() - self.start_time.unix_timestamp();
        let current_epoch = since_start / (self.duration.as_seconds_f32() as i64);
        self.start_time + self.duration.mul(current_epoch as u32)
    }

    pub(crate) fn current_epoch_end_time(&self) -> OffsetDateTime {
        self.current_epoch_start_time() + self.duration
    }

    pub(crate) fn current_epoch_numer(&self) -> i64 {
        let since_start =
            OffsetDateTime::now_utc().unix_timestamp() - self.start_time.unix_timestamp();
        since_start / self.duration.as_seconds_f32() as i64
    }

    pub(crate) async fn tick(&self) {
        let start = self.current_epoch_start_time().unix_timestamp();
        let end = self.current_epoch_end_time().unix_timestamp();
        tokio::time::sleep(Duration::seconds(end - start).try_into().unwrap()).await;
    }
}
