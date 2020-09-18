use bson::ordered::OrderedDocument;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize)]
pub struct AuthSession {
    id_token: String,
    expires_at: i64,
}

impl AuthSession {
    pub fn new(id_token: String, expires_at: i64) -> Self {
        AuthSession {
            id_token,
            expires_at,
        }
    }

    pub fn get_expires_at(&self) -> i64 {
        self.expires_at
    }
}

impl From<OrderedDocument> for AuthSession {
    fn from(document: OrderedDocument) -> Self {
        AuthSession {
            id_token: document
                .get_str("id_token")
                .expect("[Session::from] id_token unwrap failed")
                .to_string(),
            expires_at: document
                .get_i64("expires_at")
                .expect("[Session::from] expires_at unwrap failed"),
        }
    }
}

impl Into<OrderedDocument> for AuthSession {
    fn into(self) -> OrderedDocument {
        doc! {
            "id_token" => self.id_token,
            "expires_at" => self.expires_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct IdInfo {
    pub iss: String,
    pub sub: String,
    pub azp: String,
    pub aud: String,
    pub iat: String,
    pub exp: String,
    pub hd: Option<String>,
    pub email: Option<String>,
    pub email_verified: Option<String>,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub locale: Option<String>,
}
