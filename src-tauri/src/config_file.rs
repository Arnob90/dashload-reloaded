use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    auto_add_download: bool,
}
impl Default for Config {
    fn default() -> Self {
        Config {
            auto_add_download: false,
        }
    }
}
