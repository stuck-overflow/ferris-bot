use futures::executor::block_on;
use log::{debug, error, trace};
use serde::{Deserialize, Serialize};
use tiny_http::{Response, Server, StatusCode};
use url::Url;

// Public interface of this module.
/// First authentication token returned by Twitch APIs.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FirstToken {
    pub access_token: String,
    pub expires_in: i64,
    pub refresh_token: String,
}

/// Twitch authentication flow.
pub fn auth_flow(client_id: &str, client_secret: &str) -> FirstToken {
    let hook = TwitchAuthHook::new(String::from(client_id), String::from(client_secret), 10666);
    println!(
        "To obtain an authentication token, please visit\n{}",
        hook.get_twitch_auth_url()
    );
    let auth = hook.receive_auth_token().unwrap();
    hook.obtain_first_token(auth)
}

// Internal implementation.
struct TwitchAuthHook {
    client_id: String,
    client_secret: String,
    http_server: Server,
}

impl TwitchAuthHook {
    fn new(client_id: String, client_secret: String, port: i32) -> TwitchAuthHook {
        let http_server = Server::http(format!("0.0.0.0:{}", port)).unwrap();
        TwitchAuthHook {
            client_id,
            client_secret,
            http_server,
        }
    }

    fn get_twitch_auth_url(&self) -> String {
        format!(
            "https://id.twitch.tv/oauth2/authorize?client_id={}&redirect_uri=http://localhost:{}&response_type=code&scope=chat:read%20chat:edit",
            self.client_id,
            self.http_server.server_addr().port()
        )
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

    fn obtain_first_token(&self, auth_code: String) -> FirstToken {
        self.obtain_first_token_impl(auth_code, "https://id.twitch.tv/oauth2/token".to_owned())
    }

    // By default, this method is called by obtain_first_token passing the
    // Twitch authentication servers URL as `remote_http_host` parameter. For
    // testing purposes, a different `remote_http_host` with a fake
    // implementation can be passed.
    fn obtain_first_token_impl(&self, auth_code: String, remote_http_host: String) -> FirstToken {
        let url = format!("{}?code={}&client_id={}&client_secret={}&grant_type=authorization_code&redirect_uri=http://localhost:{}", &remote_http_host, &auth_code, self.client_id, self.client_secret, self.http_server.server_addr().port());
        debug!("posting to {}", url);
        let result = block_on(async {
            let client = reqwest::Client::new();
            client
                .post(&url)
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        });
        serde_json::from_str::<FirstToken>(&result).unwrap()
    }
}

// Tests.
#[cfg(test)]
mod tests {
    use reqwest::StatusCode;

    use super::*;

    #[test]
    fn twitch_auth_url() {
        let client_id = "xxxxxx".to_owned();
        let client_secret = "".to_owned();
        let hook = TwitchAuthHook::new(client_id.clone(), client_secret.clone(), 0);
        let expected_address = format!(
            "https://id.twitch.tv/oauth2/authorize?client_id={}&redirect_uri=http://localhost:{}&response_type=code&scope=chat:read%20chat:edit", 
            client_id,
            hook.http_server.server_addr().port());
        assert_eq!(hook.get_twitch_auth_url(), expected_address);
    }

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
            reqwest::get(&format!("{}/favicon.ico", http_address))
                .await
                .unwrap();
            // now the real request.
            reqwest::get(&format!(
                "{}/?code={}&scope=chat%3Aread+chat%3Aedit",
                http_address, expected_auth_code_clone
            ))
            .await
            .unwrap()
        });

        let response = testing_driver.await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let received_auth_code = received_auth_code.await.unwrap();
        assert_eq!(received_auth_code, Ok(expected_auth_code.to_owned()));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn obtain_first_token() {
        let expected_client_id = "xxxxxx".to_owned();
        let expected_client_secret = "yyyyyyyy".to_owned();
        let expected_auth_code = "zzzzzzzz".to_owned();
        let hook = TwitchAuthHook::new(
            expected_client_id.clone(),
            expected_client_secret.clone(),
            0,
        );
        let bot_port = hook.http_server.server_addr().port();
        let expected_first_token = FirstToken {
            access_token: "xxxxxx".to_owned(),
            expires_in: 123,
            refresh_token: "yyyy".to_owned(),
        };

        // Set up and start fake twitch server.
        let expected_first_token_clone = expected_first_token.clone();
        let expected_auth_code_clone = expected_auth_code.clone();
        let expected_client_id_clone = expected_client_id.clone();
        let expected_client_secret_clone = expected_client_secret.clone();
        let http_server = Server::http("0.0.0.0:0").unwrap();
        let server_port = http_server.server_addr().port();
        tokio::spawn(async move {
            match http_server.recv() {
                Ok(rq) => {
                    assert_eq!(rq.method(), &tiny_http::Method::Post);

                    let url = format!(
                        "http://localhost:{}{}",
                        http_server.server_addr().port(),
                        rq.url()
                    );
                    let url = Url::parse(&url).unwrap();
                    let pairs = url.query_pairs();
                    let mut actual_code: Option<String> = None;
                    let mut actual_client_id: Option<String> = None;
                    let mut actual_client_secret: Option<String> = None;
                    let mut actual_redirect_uri: Option<String> = None;
                    let mut actual_grant_type: Option<String> = None;
                    for (key, value) in pairs {
                        trace!("parsing keys {} = {}", key, value);
                        match &*key {
                            "code" => actual_code = Some(value.into_owned()),
                            "client_id" => actual_client_id = Some(value.into_owned()),
                            "client_secret" => actual_client_secret = Some(value.into_owned()),
                            "grant_type" => actual_grant_type = Some(value.into_owned()),
                            "redirect_uri" => actual_redirect_uri = Some(value.into_owned()),
                            _ => continue,
                        }
                    }

                    assert_eq!(Some(expected_auth_code_clone), actual_code);
                    assert_eq!(Some(expected_client_id_clone), actual_client_id);
                    assert_eq!(Some(expected_client_secret_clone), actual_client_secret);
                    assert_eq!(Some(String::from("authorization_code")), actual_grant_type);
                    let expected_redirect_uri = format!("http://localhost:{}", bot_port);
                    assert_eq!(Some(expected_redirect_uri), actual_redirect_uri);

                    let response = Response::from_string(
                        serde_json::to_string(&expected_first_token_clone).unwrap(),
                    );
                    rq.respond(response).unwrap();
                }
                Err(e) => {
                    println!("Error: {:?}", e);
                }
            };
        });

        let first_token = hook.obtain_first_token_impl(
            expected_auth_code.clone(),
            format!("http://localhost:{}", server_port),
        );
        assert_eq!(first_token, expected_first_token);
    }
}
