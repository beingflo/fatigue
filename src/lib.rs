use chrono::{Timelike, Utc};
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use reqwest;
use std::future::Future;
mod tests;

#[derive(PartialEq, Debug)]
pub enum RunResult {
    Ok,
    SlowDown,
}

pub async fn run<F, Fut>(fun: F)
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<RunResult, reqwest::Error>> + Send + 'static,
{
    let max_active = 50;

    let mut futures = FuturesUnordered::new();

    for _ in 0..max_active {
        futures.push(fun());
    }

    let mut counter_per_second = 0;
    let mut current_second = 0;

    while let Some(fut) = futures.next().await {
        fut.unwrap();

        let now = Utc::now();
        if current_second == now.second() {
            counter_per_second += 1;
        } else {
            println!("{}", counter_per_second);
            counter_per_second = 1;
            current_second = now.second();
        }

        while futures.len() < max_active {
            futures.push(fun());
        }
        // max_active += 1;
    }
}
