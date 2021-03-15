use futures::executor::block_on;
use log::{debug, error};
use tiny_http::{Response, Server, StatusCode};
use twitch_oauth2::{tokens::UserTokenBuilder, ClientId, ClientSecret, Scope, UserToken};
use url::Url;

/// Twitch authentication flow.
pub fn auth_flow(client_id: &str, client_secret: &str) -> UserToken {
    let mut hook = TwitchAuthHook::new(String::from(client_id), String::from(client_secret), 10666);
    let (url, csrf) = hook.builder.generate_url();
    println!(
        "To obtain an authentication token, please visit\n{}",
        url.as_str().to_owned()
    );
    let code = hook.receive_auth_token().unwrap();
    let user_token = block_on(async {
        hook.builder
            .get_user_token(
                twitch_oauth2::client::surf_http_client,
                csrf.secret(),
                &code,
            )
            .await
    });
    user_token.unwrap()
}

// Internal implementation.
struct TwitchAuthHook {
    http_server: Server,
    builder: UserTokenBuilder,
}

impl TwitchAuthHook {
    fn new(client_id: String, client_secret: String, port: i32) -> TwitchAuthHook {
        let http_server = Server::http(format!("0.0.0.0:{}", port)).unwrap();
        let redirect_url = oauth2::RedirectUrl::new(format!(
            "http://localhost:{}",
            http_server.server_addr().port()
        ))
        .unwrap();
        let builder = UserToken::builder(
            ClientId::new(client_id),
            ClientSecret::new(client_secret),
            redirect_url,
        )
        .unwrap()
        .force_verify(true)
        .set_scopes(vec![
            Scope::ChannelReadSubscriptions,
            Scope::ChatRead,
            Scope::ChatEdit,
        ]);
        TwitchAuthHook {
            http_server,
            builder,
        }
    }

    fn receive_auth_token(&self) -> Result<String, ()> {
        let mut code: Option<String> = None;
        loop {
            match self.http_server.recv() {
                Ok(rq) => {
                    debug!("request: {:?}", rq);
                    let url = format!(
                        "http://localhost:{}{}",
                        self.http_server.server_addr().port(),
                        rq.url()
                    );
                    let url = Url::parse(&url).unwrap();
                    if url.path() != "/" {
                        rq.respond(Response::from_string("KO").with_status_code(StatusCode(500)))
                            .unwrap();
                        continue;
                    }

                    for (key, value) in url.query_pairs() {
                        match &*key {
                            "code" => code = Some(value.into_owned()),
                            _ => continue,
                        }
                    }
                    if code != None {
                        rq.respond(Response::from_string("OK")).unwrap();
                        break;
                    } else {
                        rq.respond(Response::from_string("KO").with_status_code(StatusCode(500)))
                            .unwrap();
                        continue;
                    }
                }
                Err(e) => {
                    error!("error: {}", e)
                }
            };
        }

        match code {
            Some(c) => Ok(c),
            None => Err(()),
        }
    }
}

// Tests.
#[cfg(test)]
mod tests {
    use surf::StatusCode;

    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn receive_auth_token_redirect() {
        let client_id = "xxxxxx".to_owned();
        let client_secret = "".to_owned();
        let hook = TwitchAuthHook::new(client_id.clone(), client_secret.clone(), 0);
        let server_port = hook.http_server.server_addr().port();
        let received_auth_code = tokio::spawn(async move { hook.receive_auth_token() });

        let expected_auth_code = "XXXXXXXX";
        let expected_auth_code_clone = expected_auth_code.clone();
        let testing_driver = tokio::spawn(async move {
            let http_address = format!("http://localhost:{}", server_port);
            // make a bogus requests to ensure the server doesn't quit.
            surf::get(&format!("{}/favicon.ico", http_address))
                .await
                .unwrap();
            // now the real request.
            surf::get(&format!(
                "{}/?code={}&scope=chat%3Aread+chat%3Aedit",
                http_address, expected_auth_code_clone
            ))
            .await
            .unwrap()
        });

        let response = testing_driver.await.unwrap();
        assert_eq!(response.status(), StatusCode::Ok);
        let received_auth_code = received_auth_code.await.unwrap();
        assert_eq!(received_auth_code, Ok(expected_auth_code.to_owned()));
    }
}
