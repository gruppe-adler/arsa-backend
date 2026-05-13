use axum::{
    extract::rejection::JsonRejection,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use sea_orm::DbErr;
use std::sync::Arc;
use utoipa::ToSchema;

use crate::models::responses::ErrorResponse;

#[derive(Debug)]
pub enum ArsaError {
    SerdeError(serde_json::Error),
    DatabaseError(DbErr),
    JsonRejection(JsonRejection),
    BollardError(bollard::errors::Error),
    NotFound,
    BadRequest,
    #[allow(dead_code)]
    UnknownError,
    IOError(std::io::Error),
    FSExtra(fs_extra::error::Error),
}

impl IntoResponse for ArsaError {
    fn into_response(self) -> Response {
        dbg!(&self);
        let (status, message, err) = match &self {
            ArsaError::JsonRejection(rejection) => {
                (rejection.status(), rejection.body_text(), None)
            }
            ArsaError::DatabaseError(db_err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                db_err.to_string(),
                Some(self),
            ),
            ArsaError::NotFound => (
                StatusCode::NOT_FOUND,
                StatusCode::NOT_FOUND.to_string(),
                Some(self),
            ),
            ArsaError::UnknownError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                StatusCode::INTERNAL_SERVER_ERROR.to_string(),
                Some(self),
            ),
            ArsaError::SerdeError(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                error.to_string(),
                Some(self),
            ),
            ArsaError::BollardError(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                error.to_string(),
                Some(self),
            ),
            ArsaError::IOError(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                error.to_string(),
                Some(self),
            ),
            ArsaError::FSExtra(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                error.to_string(),
                Some(self),
            ),
            ArsaError::BadRequest => (
                StatusCode::BAD_REQUEST,
                "Bad Request".to_string(),
                Some(self),
            ),
        };

        let mut response = (status, AppJson(ErrorResponse { message })).into_response();
        if let Some(err) = err {
            response.extensions_mut().insert(Arc::new(err));
        }
        response
    }
}

#[derive(ToSchema, axum_macros::FromRequest)]
#[from_request(via(axum::Json), rejection(ArsaError))]
pub struct AppJson<T>(pub T);

impl<T> IntoResponse for AppJson<T>
where
    axum::Json<T>: IntoResponse,
{
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

impl From<JsonRejection> for ArsaError {
    fn from(rejection: JsonRejection) -> Self {
        Self::JsonRejection(rejection)
    }
}

impl From<serde_json::Error> for ArsaError {
    fn from(value: serde_json::Error) -> Self {
        Self::SerdeError(value)
    }
}

impl From<DbErr> for ArsaError {
    fn from(err: DbErr) -> Self {
        Self::DatabaseError(err)
    }
}

impl From<bollard::errors::Error> for ArsaError {
    fn from(value: bollard::errors::Error) -> Self {
        Self::BollardError(value)
    }
}

impl From<std::io::Error> for ArsaError {
    fn from(value: std::io::Error) -> Self {
        Self::IOError(value)
    }
}

impl From<fs_extra::error::Error> for ArsaError {
    fn from(value: fs_extra::error::Error) -> Self {
        Self::FSExtra(value)
    }
}
