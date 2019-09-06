use sodiumoxide::crypto::generichash::State;

pub fn digest(data: &[u8]) -> Vec<u8> {
    let mut hasher = State::new(32, None).unwrap();
    hasher.update(data).expect("Failed to update hasher state");

    let hash = hasher.finalize().unwrap();
    let mut result = Vec::new();
    result.extend_from_slice(hash.as_ref());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blake2b_256() {
        let hash = digest(b"hello world");
        let expected = hex::decode("256c83b297114d201b30179f3f0ef0cace9783622da5974326b436178aeef610").unwrap();
        assert_eq!(expected, hash)
    }
}