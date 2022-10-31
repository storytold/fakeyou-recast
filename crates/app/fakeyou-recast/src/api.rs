use std::collections::HashMap;
use crate::Message;
pub use crate::error::ApiError;

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
                reqwest::StatusCode::OK => {
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
                reqwest::StatusCode::TOO_MANY_REQUESTS => {
                    Err(ApiError::ServerOverloaded)
                }
                reqwest::StatusCode::UNAUTHORIZED => {
                    Err(ApiError::InvalidLoginCredentials)
                },
                reqwest::StatusCode::INTERNAL_SERVER_ERROR => {
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
        Err(e) => Message::LoginFailed(e),
    }
}

pub async fn check_session(token: String) -> Result<json::JsonValue, ApiError> {
    let client = reqwest::Client::builder().cookie_store(true).build().map_err(|_|ApiError::ReqwestError)?;
    let cookie_str = format!("session={}",token);
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::COOKIE, reqwest::header::HeaderValue::from_str(&cookie_str).map_err(|_|ApiError::ReqwestError)?);
    let response = client.get("https://api.fakeyou.com/api/v1/session").headers(headers).send().await.map_err(|_|ApiError::ReqwestError)?;
    match response.status() {
        reqwest::StatusCode::OK => {
            let json = response.text().await.map_err(|_|ApiError::ReqwestError)?;
            let object = json::parse(&json).map_err(|_|ApiError::Parse)?;
            if object["logged_in"].as_bool().ok_or(ApiError::Parse)? {
                let user_data = object["user"].clone();
                Ok(user_data)
            } else {
                Err(ApiError::InvalidSession)
            }
        },
        reqwest::StatusCode::TOO_MANY_REQUESTS => {
            Err(ApiError::ServerOverloaded)
        }
        reqwest::StatusCode::INTERNAL_SERVER_ERROR => {
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
        Err(e) => Message::SessionFailed(e),
    }
}
