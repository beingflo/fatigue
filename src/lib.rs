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

pub async fn run<F, S, I, T, Int, Fut, TFut>(fun: F, setup: S, teardown: T)
where
    F: Fn(I) -> Fut,
    S: Fn() -> Int,
    T: Fn(I) -> TFut,
    I: Clone,
    Int: Future<Output = I> + Send + 'static,
    Fut: Future<Output = Result<RunResult, reqwest::Error>> + Send + 'static,
    TFut: Future<Output = ()> + Send + 'static,
{
    let max_requests_in_flight = 10000;
    let mut target_requests_per_second = 1000;

    let num_clients = 10;

    let mut handles = Vec::new();

    let mut request_start = Vec::new();
    let mut request_end = Vec::new();

    let mut clients = Vec::new();
    for _ in 0..num_clients {
        clients.push(setup().await);
    }

    for i in 0..target_requests_per_second {
        handles.push(tokio::spawn(fun(clients[i % num_clients].clone())));
        request_start.push(Utc::now());
    }

    let mut last_slow_down = 0;

    while !handles.is_empty() {
        let (result, _, futs) = select_all(handles).await;
        handles = futs;

        let status = result.unwrap().unwrap();

        if status == RunResult::SlowDown
            && request_end.len() - last_slow_down > 2 * target_requests_per_second
        {
            target_requests_per_second /= 2;
            last_slow_down = request_end.len();
            println!("Slow down! -> {}", target_requests_per_second);
        }

        if request_end.len() % 1000 == 0 {
            target_requests_per_second += 100;
            println!("{}", target_requests_per_second);
        }

        if handles.is_empty() {
            let last_start = Utc::now() - request_start.last().unwrap().clone();
            let sleep_duration = time::Duration::from_secs(1) - last_start.to_std().unwrap();
            sleep(sleep_duration).await;
        }

        request_end.push(Utc::now());

        let requests_in_second = requests_in_last_second(&mut request_start);

        let num_spawn = cmp::min(
            cmp::max(
                target_requests_per_second as i64 - requests_in_second as i64,
                0,
            ),
            max_requests_in_flight - handles.len() as i64,
        );

        if request_end.len() == 1000000 {
            break;
        }

        for i in 0..num_spawn {
            handles.push(tokio::spawn(fun(clients[i as usize % num_clients].clone())));
            request_start.push(Utc::now());
        }
    }

    for c in clients.into_iter() {
        teardown(c).await;
    }
}

fn requests_in_last_second(requests: &mut Vec<DateTime<Utc>>) -> usize {
    let now = Utc::now();

    requests.retain(|&time| time > now - Duration::seconds(1));

    requests.len()
}
