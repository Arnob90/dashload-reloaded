use std::pin::Pin;
#[deprecated(
    note = "FutureVec is obsolete. Use standard unboxed Vec<impl Future> with join_all instead."
)]
pub type FutureVec<'a, OutputT> = Vec<Pin<Box<dyn Future<Output = OutputT> + 'a + Send>>>;

pub fn clamp(value: u64, min: u64, max: u64) -> u64 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}
