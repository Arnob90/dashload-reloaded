use serde::Serialize;
use thiserror::Error;
#[derive(Error, Debug, Serialize, Clone, PartialEq)]
#[serde(tag = "kind", content = "payload")]
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

pub async fn get_length_from_response(
    response: reqwest::Response,
) -> Result<u64, LengthGettingErrors> {
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

pub async fn get_length(client: reqwest::Client, url: &str) -> Result<u64, LengthGettingErrors> {
    //First try head
    //If only we could map results async
    let response = client.head(url).send().await;
    if response.is_err() {
        let try_get = client.get(url).send().await?;
        return get_length_from_response(try_get).await;
    }
    let length = get_length_from_response(response.unwrap()).await;
    if let Err(LengthGettingErrors::LengthlessServerError) = length {
        let try_get = client.get(url).send().await?;
        return get_length_from_response(try_get).await;
    }
    length
}
