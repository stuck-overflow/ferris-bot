use futures::executor::block_on;
use log::debug;
use serde::{Deserialize, Serialize};
use tiny_http::{Response, Server};
use url::Url;

struct TwitchAuthHook {
    client_id: String,
    client_secret: String,
    http_server: Server,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FirstToken {
    access_token: String,
    expires_in: i64,
    refresh_token: String,
}

impl TwitchAuthHook {
    fn new(client_id: String, client_secret: String) -> TwitchAuthHook {
        let http_server = Server::http("0.0.0.0:0").unwrap();
        TwitchAuthHook {
            client_id,
            client_secret,
            http_server,
        }
    }

    fn get_twitch_auth_url(&self) -> String {
        format!("https://id.twitch.tv/oauth2/authorize?client_id={}&redirect_uri=http://localhost:{}&response_type=code&scope=chat:read%20chat:edit", self.client_id, self.http_server.server_addr().port())
    }

    fn receive_auth_token(&self) -> Result<String, ()> {
        // wait for http request
        let mut code: Option<String> = None;
        match self.http_server.recv() {
            Ok(rq) => {
                debug!("request: {:?}", rq);
                let response = Response::from_string("OK");
                let url = format!(
                    "http://localhost:{}{}",
                    self.http_server.server_addr().port(),
                    rq.url()
                );
                let url = Url::parse(&url).unwrap();
                let mut pairs = url.query_pairs();
                loop {
                    match pairs.next() {
                        Some(pair) => {
                            println!("{} = {}", pair.0, pair.1);
                            if pair.0.eq("code") {
                                code = Some(pair.1.into_owned());
                                break;
                            }
                        }
                        None => break,
                    }
                }
                rq.respond(response).unwrap();
            }
            Err(e) => {
                println!("error: {}", e)
            }
        };

        match code {
            Some(c) => Ok(c),
            None => Err(()),
        }
    }

    fn obtain_first_token(&self, auth_code: String) -> FirstToken {
        self.obtain_first_token_impl(auth_code, "https://id.twitch.tv".to_owned())
    }

    // This function is used for testing purposes to point at a fake server.
    fn obtain_first_token_impl(&self, auth_code: String, remote_http_host: String) -> FirstToken {
        block_on(reqwest::get(&remote_http_host));
        FirstToken {
            access_token: "".to_owned(),
            expires_in: 123,
            refresh_token: "".to_owned(),
        }
    }
}

pub fn auth_flow() -> FirstToken {
    // Instantiate TwitchAuthHook
    // call get_twitch_auth_url
    // wait for user redirection on specified port
    //  auth_flow_internal(server)
    FirstToken {
        access_token: "".to_owned(),
        expires_in: 123,
        refresh_token: "".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;

    use super::*;

    #[test]
    fn twitch_auth_url() {
        let client_id = "xxxxxx".to_owned();
        let client_secret = "".to_owned();
        let hook = TwitchAuthHook::new(client_id.clone(), client_secret.clone());
        let expected_address = format!("https://id.twitch.tv/oauth2/authorize?client_id={}&redirect_uri=http://localhost:{}&response_type=code&scope=chat:read%20chat:edit", client_id, hook.http_server.server_addr().port());
        assert_eq!(hook.get_twitch_auth_url(), expected_address);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn receive_auth_token_redirect() {
        let client_id = "xxxxxx".to_owned();
        let client_secret = "".to_owned();
        let hook = TwitchAuthHook::new(client_id.clone(), client_secret.clone());
        let server_port = hook.http_server.server_addr().port();
        let received_auth_code = tokio::spawn(async move { hook.receive_auth_token() });

        let expected_auth_code = "XXXXXXXX";
        let expected_auth_code_clone = expected_auth_code.clone();
        let testing_driver = tokio::spawn(async move {
            let url = format!(
                "http://localhost:{}/?code={}&scope=chat%3Aread+chat%3Aedit",
                server_port, expected_auth_code_clone
            );
            reqwest::get(&url).await.unwrap()
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
        let expected_first_token = FirstToken {
            access_token: "xxxxxx".to_owned(),
            expires_in: 123,
            refresh_token: "yyyy".to_owned(),
        };
        let expected_first_token_clone = expected_first_token.clone();
        // start fake twitch server
        let http_server = Server::http("0.0.0.0:0").unwrap();
        let server_port = http_server.server_addr().port();
        let fake_server = tokio::spawn(async move {
            match http_server.recv() {
                Ok(rq) => {
                    println!("TWITCH FAKE SERVER: {:?}", rq);
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

        let hook = TwitchAuthHook::new(expected_client_id.clone(), expected_client_secret.clone());
        let first_token = hook.obtain_first_token_impl(
            expected_auth_code.clone(),
            format!("http://localhost:{}", server_port),
        );
        assert_eq!(first_token, expected_first_token);
    }
}
