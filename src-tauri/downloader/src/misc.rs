use std::pin::Pin;

pub type FutureVec<'a, OutputT> = Vec<Pin<Box<dyn Future<Output = OutputT> + 'a + Send>>>;
