#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use reqwest::{Client, ClientBuilder};
    use serde::Serialize;

    use crate::{run, RunResult};

    async fn f(client: Client) -> Result<RunResult, reqwest::Error> {
        let start = Utc::now();
        let response = client.get("http://localhost:3030/notes").send().await?;
        let elapsed = Utc::now() - start;
        assert_eq!(response.status(), 200);

        if elapsed > Duration::milliseconds(1000) {
            return Ok(RunResult::SlowDown);
        }

        return Ok(RunResult::Ok);
    }

    async fn setup() -> Client {
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

        client
    }

    async fn teardown(client: Client) {
        let response = client.delete("http://localhost:3030/session").send().await;

        assert!(response.is_ok());

        if let Ok(response) = response {
            assert_eq!(response.status(), 200);
        }
    }

    #[derive(Serialize)]
    struct SignupRequest {
        name: String,
        password: String,
    }

    #[tokio::test]
    async fn it_works() {
        run(f, setup, teardown).await;
    }
}
