use sha2::{Digest, Sha256};

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut result = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut result, "{byte:02x}").expect("writing to String cannot fail");
    }
    result
}

pub fn sha256_v1(bytes: &[u8]) -> String {
    format!("v1:{}", sha256_hex(bytes))
}

#[cfg(test)]
mod tests {
    use super::{sha256_hex, sha256_v1};

    #[test]
    fn hashes_are_lowercase_and_versioned() {
        assert_eq!(
            sha256_hex(b"synthetic"),
            "b3cc0475bb78a5026098858e9889acf666d31062d513d303314eca31d36e72f2"
        );
        assert_eq!(
            sha256_v1(b"synthetic"),
            "v1:b3cc0475bb78a5026098858e9889acf666d31062d513d303314eca31d36e72f2"
        );
    }
}
