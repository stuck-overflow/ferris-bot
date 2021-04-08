use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use log::debug;
use oauth2::ClientSecret;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::{fs, str};
use twitch_api2::twitch_oauth2;
use twitch_api2::twitch_oauth2::TwitchToken;
use twitch_irc::login::TokenStorage;

#[derive(Clone, Debug)]
pub struct CustomTokenStorage {
    pub token_checkpoint_file: String,
}
// Since twitch_oauth2::UserToken is not serializable, create our own
// serializable struct. This struct can be converted to either
// twitch_oauth2::UserToken or twitch_irc::login::UserAccesstoken.
#[derive(Deserialize, Serialize)]
pub struct StoredUserToken {
    /// The access token used to authenticate requests with
    access_token: oauth2::AccessToken,
    client_id: oauth2::ClientId,
    client_secret: Option<oauth2::ClientSecret>,
    /// Username of user associated with this token
    login: String,
    /// User ID of the user associated with this token
    user_id: String,
    /// The refresh token used to extend the life of this user token
    refresh_token: Option<oauth2::RefreshToken>,
    /// Expiration time
    expires_at: Option<DateTime<Utc>>,
    scopes: Option<Vec<twitch_oauth2::Scope>>,
}

impl StoredUserToken {
    fn from_twitch_oauth2_user_token(
        user_token: &twitch_oauth2::UserToken,
        client_secret: Option<oauth2::ClientSecret>,
    ) -> StoredUserToken {
        let expires_at = Utc::now() + Duration::from_std(user_token.expires_in()).unwrap();
        StoredUserToken {
            access_token: user_token.access_token.clone(),
            client_id: user_token.client_id().clone(),
            client_secret,
            login: user_token.login.clone(),
            user_id: user_token.user_id.clone(),
            refresh_token: user_token.refresh_token.clone(),
            expires_at: Some(expires_at),
            scopes: Some(user_token.scopes().to_vec()),
        }
    }

    fn to_twitch_oauth2_user_token(&self) -> twitch_oauth2::UserToken {
        let expires_in = match self.expires_at {
            Some(exp) => Some(exp.signed_duration_since(Utc::now()).to_std().unwrap()),
            None => None,
        };
        twitch_oauth2::UserToken::from_existing_unchecked(
            self.access_token.clone(),
            self.refresh_token.clone(),
            self.client_id.clone(),
            self.client_secret.clone(),
            self.login.clone(),
            self.user_id.clone(),
            self.scopes.clone(),
            expires_in,
        )
    }

    fn to_twitch_irc_user_token(&self) -> twitch_irc::login::UserAccessToken {
        let refresh_token = match &self.refresh_token {
            Some(r) => r.secret().to_owned(),
            None => "".to_owned(),
        };
        // By default the `created_at` field of the token should be set to
        // Utc::now(), however if the expiration time `expires_at` is already in
        // the past we force `created_at` and `expires_at` fields to be equal.
        // This is because the twitch_irc library doesn't handle gracefully
        // `created_at` being greater than `expires_at`.
        let now = Utc::now();
        let created_at = if let Some(exp) = self.expires_at {
            if exp < now {
                exp
            } else {
                now
            }
        } else {
            now
        };
        twitch_irc::login::UserAccessToken {
            access_token: self.access_token.secret().to_owned(),
            refresh_token,
            created_at,
            expires_at: self.expires_at,
        }
    }

    fn update_from_twitch_irc_user_token(
        mut self,
        user_access_token: &twitch_irc::login::UserAccessToken,
    ) -> Self {
        self.access_token = oauth2::AccessToken::new(user_access_token.access_token.clone());
        self.refresh_token = Some(oauth2::RefreshToken::new(
            user_access_token.refresh_token.clone(),
        ));
        self.expires_at = user_access_token.expires_at;
        self
    }
}

#[async_trait]
impl TokenStorage for CustomTokenStorage {
    type LoadError = std::io::Error; // or some other error
    type UpdateError = std::io::Error;

    async fn load_token(&mut self) -> Result<twitch_irc::login::UserAccessToken, Self::LoadError> {
        debug!("load_token called");
        match self.load_stored_token() {
            Ok(t) => Ok(t.to_twitch_irc_user_token()),
            Err(e) => Err(e),
        }
    }

    async fn update_token(
        &mut self,
        token: &twitch_irc::login::UserAccessToken,
    ) -> Result<(), Self::UpdateError> {
        debug!("update_token called");
        let stored_token = self.load_stored_token()?;
        self.write_stored_token(&stored_token.update_from_twitch_irc_user_token(token))
    }
}

impl CustomTokenStorage {
    pub fn write_twitch_oauth2_user_token(
        &self,
        token: &twitch_oauth2::UserToken,
        client_secret: Option<ClientSecret>,
    ) -> Result<(), std::io::Error> {
        let stored_token = &StoredUserToken::from_twitch_oauth2_user_token(token, client_secret);
        self.write_stored_token(stored_token)
    }

    pub fn load_twitch_oauth2_user_token(
        &self,
    ) -> Result<twitch_oauth2::UserToken, std::io::Error> {
        let token = self.load_stored_token()?;
        Ok(token.to_twitch_oauth2_user_token())
    }

    fn load_stored_token(&self) -> Result<StoredUserToken, std::io::Error> {
        let token = fs::read_to_string(&self.token_checkpoint_file)?;
        let token = serde_json::from_str::<StoredUserToken>(&(token)).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Failed to deserialize token",
            )
        })?;
        Ok(token)
    }

    fn write_stored_token(&self, stored_token: &StoredUserToken) -> Result<(), std::io::Error> {
        let serialized = serde_json::to_string(&stored_token).unwrap();
        let _ = File::create(&self.token_checkpoint_file);
        fs::write(&self.token_checkpoint_file, serialized)
            .expect("Unable to write token to checkpoint file");
        Ok(())
    }
}
