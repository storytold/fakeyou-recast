use model_serde::{model::ModelLightMetadata, serde::deserializer::ModelDeserializer};
use crate::{AppError, Message};
use futures::stream::StreamExt;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum LocalError {
    #[error("Failed to list models in the local model directory {0}.\nEnsure the directory exists and contains valid models")]
    ModelListError(std::path::PathBuf),
}

pub async fn list_models_local<P>(model_dir: P) -> Result<Vec<LocalModelListItem>, LocalError> 
    where P: AsRef<Path>
{
    let model_des = ModelDeserializer::new(crate::consts::CRYPTO_SECRET);
    let dir_list = async_fs::read_dir(model_dir.as_ref()).await.map_err(|_e| LocalError::ModelListError(model_dir.as_ref().to_path_buf()))?;
    let vec: Vec<LocalModelListItem> = dir_list
        .filter_map(|res| async move { 
            match res {
                Ok(entry) => {
                    let file_type = entry.file_type().await.ok();
                    let path = entry.path();
                    let extension = path.extension();
                    match (file_type, extension) {
                        (Some(ftype), Some(ext)) => {
                            if ftype.is_file() && ext == "fakeyou" {
                                Some(entry)
                            } else {
                                None
                            }
                        }
                        _ => None
                    }
                },
                Err(_) => None
            }
    }).map(|dirent| (dirent.path(), model_des.read_fast_model_file_metadata(dirent.path())))
    .filter_map(|(path, res)| async move {
        match res {
            Ok(meta) => {
                Some(LocalModelListItem {
                    path,
                    meta,
                })
            },
            Err(_) => None
        }
    })
    .collect().await;
    Ok(vec)
}

pub fn list_models_local_callback(res: Result<Vec<LocalModelListItem>, LocalError>) -> Message {
    match res {
        Ok(vec) => {
            Message::ListModelsLocalSuccess(vec)
        }
        Err(e) => {
           Message::ListModelsLocalFailure(e) 
        }
    }
}

#[derive(Eq, Debug, Clone)]
pub struct LocalModelListItem {
    pub path: PathBuf,
    pub meta: ModelLightMetadata
}

impl core::fmt::Display for LocalModelListItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{} by {}", self.meta.model_name, self.meta.model_author_username))
    }
}

impl PartialEq for LocalModelListItem {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path && self.meta.model_token == other.meta.model_token
    }
}
