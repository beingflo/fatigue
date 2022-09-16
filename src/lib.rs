use chrono::{DateTime, Duration, Utc};
use futures::future::select_all;
use reqwest;
use std::future::Future;
use std::{cmp, time};
use tokio::time::sleep;
mod tests;

#[derive(PartialEq, Debug)]
pub enum RunResult {
    Ok,
    SlowDown,
}

pub async fn run<F, S, T, Int, Fut>(fun: F, setup: S)
where
    F: Fn(T) -> Fut,
    S: Fn() -> Int,
    T: Clone,
    Int: Future<Output = T> + Send + 'static,
    Fut: Future<Output = Result<RunResult, reqwest::Error>> + Send + 'static,
{
    let max_requests_in_flight = 10000;
    let target_requests_per_second = 200;

    let mut handles = Vec::new();

    let mut request_start = Vec::new();
    let mut request_end = Vec::new();

    let client = setup().await;

    for _ in 0..target_requests_per_second {
        handles.push(tokio::spawn(fun(client.clone())));
        request_start.push(Utc::now());
    }

    while !handles.is_empty() {
        let (_, _, futs) = select_all(handles).await;
        handles = futs;

        if handles.is_empty() {
            let last_start = Utc::now() - request_start.last().unwrap().clone();
            let sleep_duration = time::Duration::from_secs(1) - last_start.to_std().unwrap();
            sleep(sleep_duration).await;
        }

        request_end.push(Utc::now());

        let requests_in_second = requests_in_last_second(&request_start);

        let num_spawn = cmp::min(
            cmp::max(
                target_requests_per_second as i64 - requests_in_second as i64,
                0,
            ),
            max_requests_in_flight - handles.len() as i64,
        );

        println!("{}", num_spawn);
        println!("Num: {}", request_end.len());

        for _ in 0..num_spawn {
            handles.push(tokio::spawn(fun(client.clone())));
            request_start.push(Utc::now());
        }
    }
}

fn requests_in_last_second(requests: &Vec<DateTime<Utc>>) -> usize {
    let now = Utc::now();

    requests
        .iter()
        .rev()
        .filter(|&&time| time > now - Duration::seconds(1))
        .count()
}
