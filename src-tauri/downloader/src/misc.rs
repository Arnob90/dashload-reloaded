use std::pin::Pin;
#[deprecated(
    note = "FutureVec is obsolete. Use standard unboxed Vec<impl Future> with join_all instead."
)]
pub type FutureVec<'a, OutputT> = Vec<Pin<Box<dyn Future<Output = OutputT> + 'a + Send>>>;
