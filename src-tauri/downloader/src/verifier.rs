use digest::Digest;
use std::fs::File;
use std::io::Read;
use std::marker::PhantomData;
use std::path::Path;
use thiserror::Error;
#[derive(Error, Debug)]
pub enum VerifyError {
    #[error("There has been error in IO")]
    IOError(#[from] std::io::Error),
}
pub trait Verifier: Send + Sync {
    fn verify(&self, filepath: &Path) -> Result<bool, VerifyError>;
}
pub struct DigestVerify<D>
where
    D: Digest,
{
    to_match_hash: String,
    _marker: PhantomData<D>,
}
impl<D> Verifier for DigestVerify<D>
where
    D: Digest + Send + Sync,
{
    fn verify(&self, filepath: &Path) -> Result<bool, VerifyError> {
        let mut file = File::open(filepath)?;
        let mut hasher = D::new();
        let mut buffer = [0u8; 8192];
        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }
        let result_str = hex::encode(hasher.finalize());
        Ok(result_str == self.to_match_hash)
    }
}
impl<D> DigestVerify<D>
where
    D: Digest,
{
    pub fn new(to_match_hash: String) -> Self {
        Self {
            to_match_hash,
            _marker: PhantomData,
        }
    }
}
pub struct NoopVerify;
impl Verifier for NoopVerify {
    fn verify(&self, _filepath: &Path) -> Result<bool, VerifyError> {
        Ok(true)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum HashAlgo {
    Sha256,
    Sha512,
    Md5,
}
pub struct VerifierUserInput {
    algo: HashAlgo,
    hash: String,
}
pub fn create_verifier(checksum: Option<VerifierUserInput>) -> Box<dyn Verifier> {
    match checksum {
        Some(VerifierUserInput {
            algo: HashAlgo::Sha256,
            hash,
        }) => Box::new(DigestVerify::<sha2::Sha256>::new(hash)),
        Some(VerifierUserInput {
            algo: HashAlgo::Sha512,
            hash,
        }) => Box::new(DigestVerify::<sha2::Sha512>::new(hash)),
        Some(VerifierUserInput {
            algo: HashAlgo::Md5,
            hash,
        }) => Box::new(DigestVerify::<md5::Md5>::new(hash)),
        None => Box::new(NoopVerify {}),
    }
}
