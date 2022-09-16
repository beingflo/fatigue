#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use futures::lock::Mutex;
    use reqwest::{Client, ClientBuilder};
    use serde::Serialize;

    use crate::{run, RunResult};

    async fn f(client: Arc<Mutex<Client>>) -> Result<RunResult, reqwest::Error> {
        let response = client
            .lock()
            .await
            .get("http://localhost:3030/notes")
            .send()
            .await?;
        assert_eq!(response.status(), 200);

        // println!("Latency: {}", elapsed.num_milliseconds());
        return Ok(RunResult::Ok);
    }

    #[derive(Serialize)]
    struct SignupRequest {
        name: String,
        password: String,
    }

    #[tokio::test]
    async fn it_works() {
        let client = ClientBuilder::new().cookie_store(true).build().unwrap();
        let response = client
            .post("http://localhost:3030/session")
            .json(&SignupRequest {
                name: "test".into(),
                password: "test".into(),
            })
            .send()
            .await;

        assert!(response.is_ok());

        if let Ok(response) = response {
            assert_eq!(response.status(), 200);
        }

        run(|| f(Arc::new(Mutex::new(client.clone())))).await;
    }
}
