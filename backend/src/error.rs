use rocket::{http::Status, response::Responder, serde::json};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
  #[error("BadRequest")]
  BadRequest,
  #[error("Unauthorized")]
  Unauthorized,
  #[allow(clippy::enum_variant_names)]
  #[error("InternalServerError")]
  InternalServerError,
  #[error("Conflict")]
  Conflict,
  #[error("Gone")]
  Gone,
  #[error("SerdeJson Error {source:?}")]
  SerdeJson {
    #[from]
    source: json::serde_json::Error,
  },
  #[error("Io Error {source:?}")]
  IO {
    #[from]
    source: std::io::Error,
  },
  #[error("Reqwest Error {source:?}")]
  Reqwest {
    #[from]
    source: reqwest::Error,
  },
}

impl<'r, 'o: 'r> Responder<'r, 'o> for Error {
  fn respond_to(self, request: &'r rocket::Request<'_>) -> rocket::response::Result<'o> {
    log::error!("{:?}", &self);
    match self {
      Self::Unauthorized => Status::Unauthorized.respond_to(request),
      Self::Gone => Status::Gone.respond_to(request),
      Self::InternalServerError | Self::Reqwest { .. } => {
        Status::InternalServerError.respond_to(request)
      }
      Self::Conflict => Status::Conflict.respond_to(request),
      _ => Status::BadRequest.respond_to(request),
    }
  }
}
