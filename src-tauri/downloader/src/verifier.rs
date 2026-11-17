use crate::serialize_io_error::serialize_io_error;
use digest::DynDigest;
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use thiserror::Error;

/// Errors that can occur during file hash verification.
#[derive(Error, Debug, Serialize)]
pub enum VerifyError {
    #[error("There has been error in IO")]
    IOError(#[serde(serialize_with = "serialize_io_error")] #[from] std::io::Error),
    #[error("Hash mismatch: expected {expected}, got {actual}")]
    HashMismatch {
        expected: String,
        actual: String,
    },
}

/// Supported hash algorithms for integrity verification.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum HashAlgo {
    Sha256,
    Sha512,
    Md5,
}

/// Internal verifier holding the expected hash and algorithm.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Verifier {
    to_match_hash: String,
    algo: HashAlgo,
}

impl Verifier {
    /// Verifies the file at `filepath` against the expected hash.
    /// Reads the file in 8KB chunks to compute the hash incrementally.
    /// Returns (expected_hash, actual_hash) where actual_hash is the computed hash.
    pub fn verify(&self, filepath: &Path) -> Result<(String, String), VerifyError> {
        let mut hasher: Box<dyn DynDigest> = match self.algo {
            HashAlgo::Sha256 => Box::new(sha2::Sha256::new()),
            HashAlgo::Sha512 => Box::new(sha2::Sha512::new()),
            HashAlgo::Md5 => Box::new(Md5::new()),
        };
        let mut buffer = [0u8; 8192];
        let mut file = File::open(filepath)?;
        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }
        let res = hasher.finalize_reset();
        let computed_hex = hex::encode(res);
        if computed_hex == self.to_match_hash {
            Ok((self.to_match_hash.clone(), computed_hex))
        } else {
            Err(VerifyError::HashMismatch {
                expected: self.to_match_hash.clone(),
                actual: computed_hex,
            })
        }
    }
}

/// Optional wrapper around `Verifier`, allowing downloads without checksum verification.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VerifierMaybe {
    verifier: Option<Verifier>,
}

impl VerifierMaybe {
    /// Creates a new verifier with the specified algorithm and expected hash.
    pub fn new(algo: HashAlgo, to_match_hash: String) -> Self {
        VerifierMaybe {
            verifier: Some(Verifier {
                algo,
                to_match_hash,
            }),
        }
    }

    /// Creates a `VerifierMaybe` with no verification (always passes).
    pub fn none() -> Self {
        VerifierMaybe { verifier: None }
    }

    /// Verifies the file if a verifier is present; otherwise returns `Ok(true)`.
    /// Returns Ok(true) if no verifier or verification passed, Ok(false) if hash mismatched.
    pub fn verify(&self, filepath: &Path) -> Result<bool, VerifyError> {
        if let Some(verifier) = &self.verifier {
            let (_expected, _actual) = verifier.verify(filepath)?;
            Ok(true)
        } else {
            Ok(true)
        }
    }

    /// Verifies the file and reports the computed hash on mismatch.
    ///
    /// Returns `Ok(true)` when verification passes (or no verifier is set),
    /// `Err(VerifyError::HashMismatch { expected, actual })` on a checksum
    /// mismatch, and `Err(VerifyError::IOError)` for genuine filesystem
    /// failures (never panics).
    pub fn verify_with_hash(&self, filepath: &Path) -> Result<bool, VerifyError> {
        if let Some(verifier) = &self.verifier {
            verifier.verify(filepath).map(|_| true)
        } else {
            Ok(true)
        }
    }

    /// Returns the hash algorithm if a verifier is present.
    pub fn algorithm(&self) -> Option<HashAlgo> {
        self.verifier.as_ref().map(|v| v.algo)
    }

    /// Returns the expected hash string if a verifier is present.
    pub fn expected_hash(&self) -> Option<&str> {
        self.verifier.as_ref().map(|v| v.to_match_hash.as_str())
    }

    /// Returns true if a verifier is configured.
    pub fn is_some(&self) -> bool {
        self.verifier.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    const HELLO_SHA256: &str =
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
    const HELLO_SHA512: &str =
        "309ecc489c12d6eb4cc40f50c902f2b4d0ed77ee511a7c7a9bcd3ca86d4cd86f989dd35bc5ff499670da34255b45b0cfd830e81f605dcf7dc5542e93ae9cd76f";
    const HELLO_MD5: &str = "5eb63bbbe01eeed093cb22bb8f5acdc3";

    const BIG_SHA256: &str =
        "731620161155f68e1209f22bc34a726bf5a583f40acf23ae55684b674fdbebf2";
    const BIG_SHA512: &str =
        "68fea74b36d50e9bff5ec4eca80430c82f76c234d36e82608fc47317d0eaa1b57acdc1cb6e52a35199a75bb4067de9e28373e3d8d644be265763fe7a40a6118f";
    const BIG_MD5: &str = "7354f16d06a013232daa23cfa7a092df";

    fn write_hello(file: &mut std::fs::File) {
        file.write_all(b"hello world").unwrap();
        file.flush().unwrap();
    }

    /// Pass case for all three supported algorithms.
    #[test]
    fn verify_passes_for_all_algorithms() {
        let mut file = NamedTempFile::new().unwrap();
        write_hello(file.as_file_mut());
        let path = file.path();

        let v = VerifierMaybe::new(HashAlgo::Sha256, HELLO_SHA256.to_string());
        assert!(v.verify(path).unwrap());

        let v = VerifierMaybe::new(HashAlgo::Sha512, HELLO_SHA512.to_string());
        assert!(v.verify(path).unwrap());

        let v = VerifierMaybe::new(HashAlgo::Md5, HELLO_MD5.to_string());
        assert!(v.verify(path).unwrap());
    }

    /// Mismatch returns `HashMismatch` with both digests populated.
    #[test]
    fn verify_mismatch_returns_hash_mismatch() {
        let mut file = NamedTempFile::new().unwrap();
        write_hello(file.as_file_mut());
        let path = file.path();

        let v = VerifierMaybe::new(HashAlgo::Sha256, "00".repeat(32));
        let err = v.verify_with_hash(path).unwrap_err();
        match err {
            VerifyError::HashMismatch { expected, actual } => {
                assert_eq!(expected, "00".repeat(32));
                assert_eq!(actual, HELLO_SHA256);
            }
            other => panic!("expected HashMismatch, got {other:?}"),
        }
    }

    /// Missing file -> IOError, never panics (doc contract).
    #[test]
    fn verify_missing_file_returns_io_error() {
        let v = VerifierMaybe::new(HashAlgo::Sha256, HELLO_SHA256.to_string());
        let err = v.verify_with_hash(Path::new("/nonexistent/path/does/not/exist"))
            .unwrap_err();
        assert!(matches!(err, VerifyError::IOError(_)));
    }

    /// Large file (>8 KB) exercises the chunked-read loop.
    #[test]
    fn verify_large_file_chunked() {
        let mut file = NamedTempFile::new().unwrap();
        let data: Vec<u8> = (0..100_000).map(|i| ((i * 31) + 7) as u8).collect();
        file.write_all(&data).unwrap();
        file.flush().unwrap();
        let path = file.path();

        let v = VerifierMaybe::new(HashAlgo::Sha256, BIG_SHA256.to_string());
        assert!(v.verify(path).unwrap());

        let v = VerifierMaybe::new(HashAlgo::Sha512, BIG_SHA512.to_string());
        assert!(v.verify(path).unwrap());

        let v = VerifierMaybe::new(HashAlgo::Md5, BIG_MD5.to_string());
        assert!(v.verify(path).unwrap());
    }

    /// `none()` verifier passes without touching the filesystem.
    #[test]
    fn none_verifier_passes_without_io() {
        let v = VerifierMaybe::none();
        assert!(v.verify(Path::new("/no/such/file")).unwrap());
        assert!(v.verify_with_hash(Path::new("/no/such/file")).unwrap());
    }

    /// Accessors for both the Some and None cases.
    #[test]
    fn accessors_some_and_none() {
        let some = VerifierMaybe::new(HashAlgo::Sha256, HELLO_SHA256.to_string());
        assert_eq!(some.algorithm(), Some(HashAlgo::Sha256));
        assert_eq!(some.expected_hash(), Some(HELLO_SHA256));
        assert!(some.is_some());

        let none = VerifierMaybe::none();
        assert_eq!(none.algorithm(), None);
        assert_eq!(none.expected_hash(), None);
        assert!(!none.is_some());
    }
}
