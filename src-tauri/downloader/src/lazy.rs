use std::{pin::Pin, sync::Arc};
use tokio::sync::RwLock;
type FutureReturningFunctor<T> =
    Box<dyn Fn() -> Pin<Box<dyn Future<Output = T> + Send + Sync>> + Send + Sync>;
pub struct AsyncLazy<T: Clone + Send + Sync> {
    required_value: Arc<RwLock<Option<T>>>,
    given_value_getter: FutureReturningFunctor<T>,
}
impl<T: Clone + Send + Sync> AsyncLazy<T> {
    pub fn new<F, Fut>(given_value_getter: F) -> AsyncLazy<T>
    where
        F: Fn() -> Fut,
        F: Send + Sync + 'static,
        Fut: Future<Output = T>,
        Fut: Send + Sync + 'static,
    {
        let required_functor = move || {
            let fut = given_value_getter();
            let required_fut: Pin<Box<dyn Future<Output = T> + Send + Sync>> = Box::pin(fut);
            required_fut
        };
        AsyncLazy {
            required_value: Arc::new(RwLock::new(None)),
            given_value_getter: Box::new(required_functor),
        }
    }
    pub async fn get(&self) -> T {
        //Start a new lexical block so read_lock is dropped
        {
            let read_lock = self.required_value.read().await;
            if read_lock.is_some() {
                return read_lock.clone().unwrap();
            }
        }
        let mut write_lock = self.required_value.write().await;
        //Did someone write while we tried to get it?
        if write_lock.is_some() {
            return write_lock.clone().unwrap();
        }
        let value = (self.given_value_getter)().await;
        *write_lock = Some(value.clone());
        value
    }
}
