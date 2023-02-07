use sha3::Digest;

pub struct Hasher {}

impl Hasher {
    /// Do a keccak 256-bit hash and return result.
    pub fn keccak_256(data: &[u8]) -> [u8; 32] {
        let mut output = [0u8; 32];
        output.copy_from_slice(sha3::Keccak256::digest(data).as_slice());
        output
    }
}
