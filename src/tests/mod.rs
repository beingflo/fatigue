#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::Utc;
    use futures::lock::Mutex;
    use reqwest::{Client, ClientBuilder};
    use serde::Serialize;

    use crate::{run, RunResult};

    async fn f(client: Arc<Mutex<Client>>) -> Result<RunResult, reqwest::Error> {
        let start = Utc::now();
        let response = client
            .lock()
            .await
            .get("https://api.stage.fieldnotes.land/notes")
            .send()
            .await?;
        assert!(response.status() == 200);
        let elapsed = Utc::now() - start;

        //println!("{}", elapsed.num_milliseconds());
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
            .post("https://api.stage.fieldnotes.land/session")
            .json(&SignupRequest {
                name: "test".into(),
                password: "test".into(),
            })
            .send()
            .await;

        assert!(response.is_ok());

        if let Ok(response) = response {
            assert!(response.status() == 200);
        }

        run(|| f(Arc::new(Mutex::new(client.clone())))).await;
    }
}
