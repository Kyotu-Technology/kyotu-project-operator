use chrono::{Duration, Utc};
use reqwest::Client;
use serde_json::json;

#[derive(Clone)]
pub struct Gitlab {
    pub client: Client,
    pub gitlab_addr: String,
    pub token: String,
}

impl std::fmt::Debug for Gitlab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gitlab")
            .field("gitlab_addr", &self.gitlab_addr)
            .field("token", &self.token)
            .finish()
    }
}

impl Gitlab {
    pub fn new(gitlab_addr: String, token: String) -> Self {
        Self {
            client: Client::new(),
            gitlab_addr,
            token,
        }
    }

    pub async fn get_group_by_name(&self, name: &str) -> Result<Option<u64>, reqwest::Error> {
        let url = format!("{}/api/v4/groups", &self.gitlab_addr);
        let res = self
            .client
            .get(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .query(&[("search", name)])
            .send()
            .await;
        match res {
            Ok(r) => {
                let body = r.text().await.unwrap();
                let json: serde_json::Value = serde_json::from_str(&body).unwrap();
                if json.as_array().unwrap_or(&Vec::new()).is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(json[0]["id"].as_u64().unwrap()))
                }
            }
            Err(e) => Err(e),
        }
    }

    pub async fn create_group(&self, name: &str) -> Result<u64, reqwest::Error> {
        let res = self.get_group_by_name(name).await;

        match res {
            Ok(r) => match r {
                Some(id) => {
                    log::info!("Group {} already exists", name);
                    Ok(id)
                }
                None => {
                    let url = format!("{}/api/v4/groups", &self.gitlab_addr);
                    let res = self
                        .client
                        .post(&url)
                        .header("PRIVATE-TOKEN", &self.token)
                        .json(&json!({
                            "name": name,
                            "path": name,
                            "visibility": "private",
                        }))
                        .send()
                        .await;
                    match res {
                        Ok(r) => {
                            let body = r.text().await.unwrap();
                            let json: serde_json::Value = serde_json::from_str(&body).unwrap();
                            let id = json["id"].as_u64().unwrap();
                            log::info!("Created group: {}", name);
                            Ok(id)
                        }
                        Err(e) => {
                            log::error!("Failed to create group: {:?}", e);
                            Err(e)
                        }
                    }
                }
            },
            Err(e) => {
                log::error!("Failed to create group: {:?}", e);
                Err(e)
            }
        }
    }

    #[allow(dead_code)]
    pub async fn delete_group(&self, name: &str) -> Result<String, reqwest::Error> {
        let res = self.get_group_by_name(name).await;

        match res {
            Ok(r) => {
                match r {
                    Some(id) => {
                        // delete group
                        let url = format!("{}/api/v4/groups/{}", &self.gitlab_addr, id);
                        let res = self
                            .client
                            .delete(&url)
                            .header("PRIVATE-TOKEN", &self.token)
                            .send()
                            .await;
                        match res {
                            Ok(_) => {
                                log::info!("Deleted group: {}", name);
                                Ok(name.to_string())
                            }
                            Err(e) => {
                                log::error!("Failed to delete group: {:?}", e);
                                Err(e)
                            }
                        }
                    }
                    None => {
                        log::info!("Group {} does not exist", name);
                        Ok("".to_string())
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to delete group: {:?}", e);
                Err(e)
            }
        }
    }

    pub async fn get_group_access_token_id(
        &self,
        name: &str,
        group_id: &u64,
    ) -> Result<Option<u64>, reqwest::Error> {
        let url = format!(
            "{}/api/v4/groups/{}/access_tokens",
            &self.gitlab_addr, group_id
        );
        let res = self
            .client
            .get(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .send()
            .await;
        match res {
            Ok(r) => {
                let body = r.text().await.unwrap();
                let json: serde_json::Value = serde_json::from_str(&body).unwrap();
                //iterate over the array and find the access token with the name
                for i in 0..json.as_array().unwrap_or(&Vec::new()).len() {
                    if json[i]["name"].as_str().unwrap() == name {
                        return Ok(Some(json[i]["id"].as_u64().unwrap()));
                    }
                }
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    pub async fn create_group_access_token(
        &self,
        name: &str,
        group_id: &u64,
    ) -> Result<String, reqwest::Error> {
        let url = format!(
            "{}/api/v4/groups/{}/access_tokens",
            &self.gitlab_addr, group_id
        );

        let date = Utc::now() + Duration::days(365);

        let res = self
            .client
            .post(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .json(&json!({
                "name": name,
                "scopes": ["read_registry"],
                "expires_at": date.format("%Y-%m-%d").to_string(),
            }))
            .send()
            .await;

        match res {
            Ok(r) => {
                let body = r.text().await.unwrap();
                log::info!("Response: {}", body);
                let json: serde_json::Value = serde_json::from_str(&body).unwrap();
                let token = json["token"].as_str().unwrap();
                log::info!("Created group access token: {}", name);
                Ok(token.to_string())
            }
            Err(e) => {
                log::error!("Failed to create group access token: {:?}", e);
                Err(e)
            }
        }
    }

    pub async fn delete_group_access_token(
        &self,

        name: &str,
        group_id: &u64,
    ) -> Result<String, reqwest::Error> {
        let res = self.get_group_access_token_id(name, group_id).await;

        match res {
            Ok(r) => match r {
                Some(id) => {
                    // delete group access token
                    let url = format!(
                        "{}/api/v4/groups/{}/access_tokens/{}",
                        &self.gitlab_addr, group_id, id
                    );
                    let res = self
                        .client
                        .delete(&url)
                        .header("PRIVATE-TOKEN", &self.token)
                        .send()
                        .await;
                    match res {
                        Ok(_) => {
                            log::info!("Deleted group access token: {}", name);
                            Ok(name.to_string())
                        }
                        Err(e) => {
                            log::error!("Failed to delete group access token: {:?}", e);
                            Err(e)
                        }
                    }
                }
                None => {
                    log::info!("Group access token {} does not exist", name);
                    Ok("".to_string())
                }
            },
            Err(e) => {
                log::error!("Failed to delete group access token: {:?}", e);
                Err(e)
            }
        }
    }

    pub async fn rotate_group_access_token(
        &self,
        name: &str,
        group_id: &u64,
    ) -> Result<String, reqwest::Error> {
        let token_id = self.get_group_access_token_id(name, group_id).await;
        match token_id {
            Ok(r) => match r {
                Some(_) => {
                    self.delete_group_access_token(name, group_id).await?;
                    let token = self.create_group_access_token(name, group_id).await?;
                    log::info!("Rotated group access token: {}", name);
                    Ok(token)
                }
                None => {
                    log::info!("Group access token {} does not exist", name);
                    Ok("".to_string())
                }
            },
            Err(e) => {
                log::error!("Failed to rotate group access token: {:?}", e);
                Err(e)
            }
        }
    }
}

//test create group
#[cfg(test)]
mod tests {
    use super::*;

    // test get group by name
    #[tokio::test]
    async fn test_get_group_by_name() {
        let mut server = mockito::Server::new_async().await;
        let host = server.host_with_port();

        server
            .mock("GET", "/api/v4/groups?search=test")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[]"#)
            .create();

        let gitlab = Gitlab::new(format!("http://{host}"), "test".to_string());
        let res = gitlab.get_group_by_name("test").await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    // test create group
    async fn test_create_group() {
        let mut server = mockito::Server::new_async().await;
        let host = server.host_with_port();

        server
            .mock("GET", "/api/v4/groups?search=test")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[]"#)
            .create();

        server
            .mock("POST", "/api/v4/groups")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": 1, "name": "test", "path": "test"}"#)
            .create();

        let gitlab = Gitlab::new(format!("http://{host}"), "test".to_string());
        let res = gitlab.create_group("test").await;
        assert_eq!(res.unwrap_or(0), 1);
    }

    // test delete group
    #[tokio::test]
    async fn test_delete_group() {
        let mut server = mockito::Server::new_async().await;
        let host = server.host_with_port();

        server
            .mock("GET", "/api/v4/groups?search=test")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"id": 1, "name": "test", "path": "test"}]"#)
            .create();

        server
            .mock("DELETE", "/api/v4/groups/1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": 1, "name": "test", "path": "test"}"#)
            .create();

        let gitlab = Gitlab::new(format!("http://{host}"), "test".to_string());
        let res = gitlab.delete_group("test").await;
        assert_eq!(res.unwrap_or("".to_string()), "test".to_string());
    }

    #[tokio::test]
    // test get group access token
    async fn test_get_group_access_token_id() {
        let mut server = mockito::Server::new_async().await;
        let host = server.host_with_port();

        server
            .mock(
                "GET",
                "/api/v4/groups/1/access_tokens",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"id":120,"name":"test","scopes":["read_registry"]},{"id":121,"name":"non-test","scopes":["read_registry"]}]"#)
            .create();

        let gitlab = Gitlab::new(format!("http://{host}"), "test".to_string());
        let res = gitlab.get_group_access_token_id("test", &1).await.unwrap();
        assert_eq!(res.unwrap_or(0), 120);
    }

    #[tokio::test]
    // test create group access token
    async fn test_create_group_token() {
        let mut server = mockito::Server::new_async().await;
        let host = server.host_with_port();

        server
            .mock("POST", "/api/v4/groups/1/access_tokens")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": 1, "token": "test"}"#)
            .create();

        let gitlab = Gitlab::new(format!("http://{host}"), "test".to_string());
        let res = gitlab.create_group_access_token("test", &1).await;
        assert_eq!(res.unwrap_or("".to_string()), "test".to_string());
    }

    #[tokio::test]
    // test delete group access token
    async fn test_delete_group_token() {
        let mut server = mockito::Server::new_async().await;
        let host = server.host_with_port();

        server
            .mock(
                "GET",
                "/api/v4/groups/1/access_tokens",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"id":120,"name":"test","scopes":["read_registry"]},{"id":121,"name":"non-test","scopes":["read_registry"]}]"#)
            .create();

        server
            .mock("DELETE", "/api/v4/groups/1/access_tokens/1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": 1, "token": "test"}"#)
            .create();

        let gitlab = Gitlab::new(format!("http://{host}"), "test".to_string());
        let res = gitlab.delete_group_access_token("test", &1).await;
        assert_eq!(res.unwrap_or("".to_string()), "test".to_string());
    }

    #[tokio::test]
    //test rotate group access token
    async fn test_rotate_group_access_token() {
        let mut server = mockito::Server::new_async().await;
        let host = server.host_with_port();

        server
            .mock(
                "GET",
                "/api/v4/groups/1/access_tokens",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"id":120,"name":"test","scopes":["read_registry"]},{"id":121,"name":"non-test","scopes":["read_registry"]}]"#)
            .create();

        server
            .mock("DELETE", "/api/v4/groups/1/access_tokens/1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": 1, "token": "test"}"#)
            .create();

        server
            .mock("POST", "/api/v4/groups/1/access_tokens")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": 1, "token": "test"}"#)
            .create();

        let gitlab = Gitlab::new(format!("http://{host}"), "test".to_string());
        let res = gitlab.rotate_group_access_token("test", &1).await;
        assert_eq!(res.unwrap_or("".to_string()), "test".to_string());
    }
}
