#[derive(Debug)]
pub(crate) struct MixnodeResult {
    pub(crate) mix_id: u32,
    pub(crate) reliability: u8,
}

// value in range 0-100
#[derive(Clone, Copy, Debug, Default)]
pub struct Uptime(u8);
