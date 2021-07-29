use crate::server::ServerData;

use actix_session::{Session, UserSession};
use actix_web::{
    dev::{RequestHead, Service, ServiceRequest, ServiceResponse, Transform},
    http::header,
    web::Data,
    Error,
};
use bson::{oid::ObjectId, Document};
use futures::future::{ok, Future, Ready};
use smmdb_auth::{AuthSession, Identity};
use std::{
    convert::TryFrom,
    pin::Pin,
    task::{Context, Poll},
};

pub struct Auth;

impl<S, B> Transform<S> for Auth
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = AuthMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(AuthMiddleware { service })
    }
}

pub struct AuthMiddleware<S> {
    service: S,
}

impl<S, B> Service for AuthMiddleware<S>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    #[allow(clippy::type_complexity)]
    type Future = Pin<Box<dyn Future<Output = Result<S::Response, S::Error>>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, mut req: ServiceRequest) -> Self::Future {
        let session = req.get_session();
        let data: Option<&Data<ServerData>> = req.app_data();
        if let Some(data) = data {
            if let Ok(auth_req) = AuthReq::try_from(session) {
                let expires_at = auth_req.session.as_ref().unwrap().get_expires_at();
                if let Some(account) = data.get_account_from_auth(auth_req) {
                    if !account.is_expired(expires_at) {
                        Identity::set_identity(account, &mut req);
                    }
                }
            } else if let Ok(auth_req) = AuthReq::try_from(req.head()) {
                if let Some(account) = data.get_account_from_auth(auth_req) {
                    Identity::set_identity(account, &mut req);
                }
            }
        }

        let fut = self.service.call(req);
        Box::pin(async move {
            let res = fut.await?;
            Ok(res)
        })
    }
}

#[derive(Debug)]
pub struct AuthReq {
    account_id: Option<ObjectId>,
    apikey: Option<String>,
    session: Option<AuthSession>,
}

impl TryFrom<Session> for AuthReq {
    type Error = ();

    fn try_from(session: Session) -> Result<Self, Self::Error> {
        if let (Ok(Some(account_id)), Ok(Some(id_token)), Ok(Some(expires_at))) = (
            session.get::<String>("account_id"),
            session.get::<String>("id_token"),
            session.get::<i64>("expires_at"),
        ) {
            if let Ok(account_id) = ObjectId::with_string(&account_id) {
                return Ok(AuthReq {
                    account_id: Some(account_id),
                    apikey: None,
                    session: Some(AuthSession::new(id_token, expires_at)),
                });
            }
        }
        Err(())
    }
}

impl TryFrom<&RequestHead> for AuthReq {
    type Error = ();

    fn try_from(header: &RequestHead) -> Result<Self, Self::Error> {
        if let Some(authorization) = header.headers().get(header::AUTHORIZATION) {
            if let Ok(authorization) = authorization.to_str() {
                let s: Vec<&str> = authorization.split(' ').collect();
                if let (Some("APIKEY"), Some(apikey)) = (s.get(0).copied(), s.get(1)) {
                    return Ok(AuthReq {
                        account_id: None,
                        apikey: Some((*apikey).to_string()),
                        session: None,
                    });
                }
            }
        }
        Err(())
    }
}

impl From<AuthReq> for Document {
    fn from(val: AuthReq) -> Self {
        let mut doc = doc! {};
        if let Some(account_id) = val.account_id {
            doc.insert("_id", account_id);
        }
        if let Some(session) = val.session {
            let session: Document = session.into();
            doc.insert("session", session);
        }
        if let Some(apikey) = val.apikey {
            doc.insert("apikey", apikey);
        }
        doc
    }
}
