use serde::Serialize;
use thiserror::Error;
#[derive(Error, Debug,Serialize)]
#[serde(tag="kind", content="payload")]
pub enum LengthGettingErrors {
    #[error("Request failed!")]
    RequestError(String),
    #[error("The server does not have a concept of filesize")]
    LengthlessServerError,
    #[error("The server violates basic protocols")]
    IllBehavedServer,
}
impl From<reqwest::Error> for LengthGettingErrors {
    fn from(value: reqwest::Error) -> Self {
        Self::RequestError(value.to_string())
    }
}

pub async fn get_length(client: reqwest::Client, url: &str) -> Result<u64, LengthGettingErrors> {
    let response = client.get(url).send().await?;
    let length_str = response
        .headers()
        .get("Content-Length")
        .ok_or(LengthGettingErrors::LengthlessServerError)?
        .to_str()
        .map_err(|_| LengthGettingErrors::IllBehavedServer)?;

    let length_num: u64 = length_str
        .parse()
        .map_err(|_| LengthGettingErrors::IllBehavedServer)?;
    Ok(length_num)
}
