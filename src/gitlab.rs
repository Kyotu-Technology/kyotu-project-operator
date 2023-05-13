use reqwest::Client;
use serde_json::json;

pub async fn create_group(
    url: &str,
    token: &str,
    client: &Client,
    name: &str,
) -> Result<u64, reqwest::Error> {
    let url = format!("{}/api/v4/groups", &url);
    let res = client
        .get(&url)
        .header("PRIVATE-TOKEN", token)
        .query(&[("search", name)])
        .send()
        .await;
    match res {
        Ok(r) => {
            let body = r.text().await.unwrap();
            let json: serde_json::Value = serde_json::from_str(&body).unwrap();
            if json.as_array().unwrap_or(&Vec::new()).is_empty() {
                // create group
                let res = client
                    .post(&url)
                    .header("PRIVATE-TOKEN", token)
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
            } else {
                let id = json[0]["id"].as_u64().unwrap();
                log::info!("Group {} already exists", name);
                Ok(id)
            }
        }
        Err(e) => {
            log::error!("Failed to create group: {:?}", e);
            Err(e)
        }
    }
}

//delete group
pub async fn delete_group(
    url: &str,
    token: &str,
    client: &Client,
    name: &str,
) -> Result<String, reqwest::Error> {
    let url = format!("{}/api/v4/groups", &url);
    let res = client
        .get(&url)
        .header("PRIVATE-TOKEN", token)
        .query(&[("search", name)])
        .send()
        .await;
    match res {
        Ok(r) => {
            let body = r.text().await.unwrap();
            let json: serde_json::Value = serde_json::from_str(&body).unwrap();
            if json.as_array().unwrap_or(&Vec::new()).is_empty() {
                log::info!("Group {} does not exist", name);
                return Ok(name.to_string());
            } else {
                let id = json[0]["id"].as_u64().unwrap();
                // delete group
                let url = format!("{}/{}", &url, id);
                let res = client
                    .delete(&url)
                    .header("PRIVATE-TOKEN", token)
                    .send()
                    .await;
                match res {
                    Ok(r) => {
                        log::info!("Deleted group: {}", name);
                        println!("Response: {:?}", r);
                        Ok(name.to_string())
                    }
                    Err(e) => {
                        log::error!("Failed to delete group: {:?}", e);
                        Err(e)
                    }
                }
            }
        }
        Err(e) => {
            log::error!("Failed to delete group: {:?}", e);
            Err(e)
        }
    }
}

//create group access token
pub async fn create_group_access_token(
    url: &str,
    token: &str,
    client: &Client,
    name: &str,
    id: &u64,
) -> Result<String, reqwest::Error> {
    let url = format!("{}/api/v4/groups/{}/access_tokens", &url, id);
    let res = client
        .get(&url)
        .header("PRIVATE-TOKEN", token)
        .query(&[("name", format!("{}-image-puller", name))])
        .send()
        .await;
    match res {
        Ok(r) => {
            let body = r.text().await.unwrap();
            let json: serde_json::Value = serde_json::from_str(&body).unwrap();
            if json.as_array().unwrap_or(&Vec::new()).is_empty() {
                // create group access token
                let res = client
                    .post(&url)
                    .header("PRIVATE-TOKEN", token)
                    .json(&json!({
                        "name": format!("{}-image-puller", name),
                        "scopes": ["read_registry"]
                    }))
                    .send()
                    .await;
                match res {
                    Ok(r) => {
                        let body = r.text().await.unwrap();
                        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
                        let token = json["token"].as_str().unwrap();
                        log::info!(
                            "Created group access token: {}",
                            format!("{}-image-puller", name)
                        );
                        Ok(token.to_string())
                    }
                    Err(e) => {
                        log::error!("Failed to create group access token: {:?}", e);
                        Err(e)
                    }
                }
            } else {
                let token = json[0]["id"].as_u64().unwrap();
                log::info!(
                    "Group access token {} already exists",
                    format!("{}-image-puller", name)
                );
                Ok(token.to_string())
                // delete group access token
            }
        }
        Err(e) => {
            log::error!("Failed to create group access token: {:?}", e);
            Err(e)
        }
    }
}

//test create group
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    // test create group
    async fn test_create_group() {
        let mut server = mockito::Server::new_with_port_async(8081).await;
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

        let client = reqwest::Client::new();
        let res = create_group(
            &format!("http://{}", host).to_string(),
            "test",
            &client,
            "test",
        )
        .await;
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

        let client = reqwest::Client::new();
        let res = delete_group(
            &format!("http://{}", host).to_string(),
            "test",
            &client,
            "test",
        )
        .await;
        assert_eq!(res.unwrap_or("".to_string()), "test".to_string());
    }

    #[tokio::test]
    // test create group access token
    async fn test_create_group_token() {
        let mut server = mockito::Server::new_async().await;
        let host = server.host_with_port();

        server
            .mock(
                "GET",
                "/api/v4/groups/1/access_tokens?name=test-image-puller",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[]"#)
            .create();

        server
            .mock("POST", "/api/v4/groups/1/access_tokens")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": 1, "token": "test"}"#)
            .create();

        std::env::set_var("GITLAB_URL", format!("http://{}", host));
        std::env::set_var("GITLAB_TOKEN", "test");
        let client = reqwest::Client::new();
        let res = create_group_access_token(
            &format!("http://{}", host).to_string(),
            "test",
            &client,
            "test",
            &1,
        )
        .await;
        assert_eq!(res.unwrap_or("".to_string()), "test".to_string());
    }
}
