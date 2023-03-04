use digest::consts::U32;
use digest::Digest;

pub fn blake2_256(data: &[u8]) -> [u8; 32] {
    let mut dest = [0; 32];
    type Blake2b256 = blake2::Blake2b<U32>;
    dest.copy_from_slice(Blake2b256::digest(data).as_slice());
    dest
}
