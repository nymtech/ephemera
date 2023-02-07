pub fn to_hex<T: AsRef<[u8]>>(data: T) -> String {
    array_bytes::bytes2hex("", data.as_ref())
}
