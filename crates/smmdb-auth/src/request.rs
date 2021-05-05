use crate::IdInfo;

use bson::ordered::OrderedDocument;
use serde::Serialize;
use std::convert::TryFrom;
use thiserror::Error;

#[derive(Clone, Debug, Serialize)]
pub struct AccountReq {
    pub googleid: String,
    pub username: String,
    pub email: String,
}

impl AccountReq {
    pub fn into_ordered_document(self) -> OrderedDocument {
        doc! {
            "googleid" => self.googleid,
            "username" => self.username,
            "email" => self.email,
        }
    }

    pub fn as_find(&self) -> OrderedDocument {
        doc! {
            "googleid" => self.googleid.clone()
        }
    }
}

impl TryFrom<IdInfo> for AccountReq {
    type Error = AccountConvertError;

    fn try_from(id_info: IdInfo) -> Result<Self, Self::Error> {
        let email = id_info.email.ok_or(AccountConvertError::EmailMissing)?;
        let email_verified = id_info
            .email_verified
            .ok_or(AccountConvertError::EmailNotVerified)?;
        if email_verified != "true" {
            return Err(AccountConvertError::EmailNotVerified);
        }
        let username = email
            .split('@')
            .next()
            .ok_or(AccountConvertError::EmailParsingFailed)?;
        Ok(AccountReq {
            googleid: id_info.sub,
            username: username.to_owned(),
            email,
        })
    }
}

#[derive(Debug, Error)]
pub enum AccountConvertError {
    #[error("email missing in OAuth2 response")]
    EmailMissing,
    #[error("email not verified")]
    EmailNotVerified,
    #[error("parsing username from email failed")]
    EmailParsingFailed,
}
