use thiserror::Error;
use std::collections::HashMap;
use crate::Message;
use chrono::{DateTime, Utc};
use reqwest::{StatusCode};

#[derive(Error, Debug, Clone)]
pub enum ApiError {
    #[error("Incorrect username or password entered.")]
    InvalidLoginCredentials,
    #[error("Invalid/expired session token.")]
    InvalidSession,
    #[error("Server failed to process request.")]
    ServerError,
    #[error("Unexpected error:\n{0})")]
    UnknownError(String),
    #[error("Could not send request. Ensure your computer is connected to the internet.")]
    ReqwestError/*(#[source] reqwest::Error)*/,
    #[error("Server could not process request due to too much traffic.")]
    ServerOverloaded,
    #[error("The server returned data we aren't able to parse.")]
    Parse,
}

pub async fn login(username: String, password: String) -> Result<String, ApiError> {
    let client = reqwest::Client::new();
    let mut map = HashMap::new();
    map.insert("username_or_email", username);
    map.insert("password", password);
    let res = client.post("https://api.fakeyou.com/api/v1/login")
        .json(&map)
        .send()
        .await;
    match res {
        Ok(response) => {
            match response.status() {
                StatusCode::OK => {
                    let cookie = response.cookies().next();
                    match cookie {
                        Some(cookie) => {
                            Ok(cookie.value().to_string())
                        }
                        None => {
                            Err(ApiError::Parse)
                        }
                    }
                },
                StatusCode::TOO_MANY_REQUESTS => {
                    Err(ApiError::ServerOverloaded)
                }
                StatusCode::UNAUTHORIZED => {
                    Err(ApiError::InvalidLoginCredentials)
                },
                StatusCode::INTERNAL_SERVER_ERROR => {
                    Err(ApiError::ServerError)
                },
                _ => {
                    let mut err_msg = String::new();
                    let json = response.text().await.unwrap_or(String::new());
                    let json_res = json::parse(&json);
                    match json_res {
                        Ok(object) => {
                            err_msg = String::from(object["error_message"].as_str().unwrap_or(""));
                        },
                        Err(_e) => (),
                    };
                    Err(ApiError::UnknownError(err_msg))
                }
            }
        },
        Err(_e) => {
            Err(ApiError::ReqwestError)
        }
    }
}

pub fn login_callback(res: Result<String, ApiError>) -> Message {
    match res {
        Ok(token) => Message::LoginSuccess(token),
        Err(e) => Message::LoginFailure(e),
    }
}

pub async fn check_session(token: String) -> Result<json::JsonValue, ApiError> {
    let client = reqwest::Client::builder().cookie_store(true).build().map_err(|_|ApiError::ReqwestError)?;
    let cookie_str = format!("session={}",token);
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::COOKIE, reqwest::header::HeaderValue::from_str(&cookie_str).map_err(|_|ApiError::ReqwestError)?);
    let response = client.get("https://api.fakeyou.com/api/v1/session").headers(headers).send().await.map_err(|_|ApiError::ReqwestError)?;
    match response.status() {
        StatusCode::OK => {
            let json = response.text().await.map_err(|_|ApiError::ReqwestError)?;
            let object = json::parse(&json).map_err(|_|ApiError::Parse)?;
            if object["logged_in"].as_bool().ok_or(ApiError::Parse)? {
                let user_data = object["user"].clone();
                Ok(user_data)
            } else {
                Err(ApiError::InvalidSession)
            }
        },
        StatusCode::TOO_MANY_REQUESTS => {
            Err(ApiError::ServerOverloaded)
        }
        StatusCode::INTERNAL_SERVER_ERROR => {
            Err(ApiError::ServerError)
        },
        _ => {
            let mut err_msg = String::new();
            let json = response.text().await.unwrap_or(String::new());
            let json_res = json::parse(&json);
            match json_res {
                Ok(object) => {
                    err_msg = String::from(object["error_message"].as_str().unwrap_or(""));
                },
                Err(_e) => (),
            };
            Err(ApiError::UnknownError(err_msg))
        }
 
    }
}

pub fn session_callback(res: Result<json::JsonValue, ApiError>) -> Message {
    match res {
        Ok(user) => Message::SessionValid(user),
        Err(e) => Message::SessionFailure(e),
    }
}

#[derive(Debug, Clone, Eq)]
pub struct AppModelDownloadsItem {
    // Name of the model
    pub title: String,
    pub model_id: String,

    // Where the file can be downloaded.
    pub download_url: String,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // Semi-semver. "1.0", "1.5", etc.
    // pub version_string: String,

    // Monotonic version number.
    pub version: u64,
}

impl PartialEq for AppModelDownloadsItem {
    fn eq(&self, other: &Self) -> bool {
        //self.model_id == other.model_id && self.version == other.version
        //FIXME
        false
    }
}

impl core::fmt::Display for AppModelDownloadsItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.title)
    }
}

pub async fn list_models_server(/*token: String*/) -> Result<Vec<AppModelDownloadsItem>, ApiError> {
    // thought about using serde but it would cause any api change to break the app
    // I think a bit of resiliency to api changes outweighs the benefits of
    // automatic deserialization
    
    
    //TODO will this eventually need a session token to access?

    let client = reqwest::Client::new();
    let response = client.get("https://api.fakeyou.com/api/v1/vc/downloads").send().await.map_err(|_|ApiError::ReqwestError)?;

    match response.status() {
        StatusCode::OK => {
            let json = response.text().await.map_err(|_| ApiError::ReqwestError)?;
            let object = json::parse(&json).map_err(|_| ApiError::Parse)?;
            let mut result_vec = Vec::new();
            for item in object["models"].members() {
                let title = item["title"].as_str().ok_or(ApiError::Parse)?.to_string(); 
                let download_url = item["download_url"].as_str().ok_or(ApiError::Parse)?.to_string();
                let created_date_str = item["created_at"].as_str().ok_or(ApiError::Parse)?;
                let updated_date_str = item["updated_at"].as_str().ok_or(ApiError::Parse)?; 
                let created_at = created_date_str.parse::<DateTime<Utc>>().map_err(|_|ApiError::Parse)?;
                let updated_at = updated_date_str.parse::<DateTime<Utc>>().map_err(|_|ApiError::Parse)?;

                // FIXME hardcoded some fields that the api doesn't implement yet
                let parsed_item = AppModelDownloadsItem {
                    title,
                    model_id: String::from("vcm_1234567890"),
                    download_url,
                    created_at,
                    updated_at,
                    version: 1,
                };
                result_vec.push(parsed_item); 
            }

            Ok(result_vec)
        }
        StatusCode::TOO_MANY_REQUESTS => {
            Err(ApiError::ServerOverloaded)
        }
        StatusCode::INTERNAL_SERVER_ERROR => {
            Err(ApiError::ServerError)
        },
        _ => {
            let mut err_msg = String::new();
            let json = response.text().await.unwrap_or(String::new());
            let json_res = json::parse(&json);
            match json_res {
                Ok(object) => {
                    err_msg = String::from(object["error_message"].as_str().unwrap_or(""));
                },
                Err(_e) => (),
            };
            Err(ApiError::UnknownError(err_msg))
        }
    }
}

pub fn list_models_server_callback(res: Result<Vec<AppModelDownloadsItem>, ApiError>) -> Message {
    match res {
        Ok(vec) => {
            Message::ListModelsServerSuccess(vec)
        }
        Err(e) => {
            Message::ListModelsServerFailure(e)
        }
    }
}
