use chrono::{Timelike, Utc};
use crossterm::{
    event::{self, poll, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::{stream::FuturesUnordered, Future, StreamExt};
use reqwest;
use std::{
    cmp, io, thread,
    time::{self, Duration, Instant},
};
use tokio::{sync::mpsc, time::sleep};
use tui::{
    backend::CrosstermBackend,
    style::{Color, Style},
    symbols,
    text::Span,
    widgets::{Axis, Block, Chart, Dataset, GraphType},
    Terminal,
};
mod tests;

#[derive(PartialEq, Debug)]
pub enum RunResult {
    Ok(chrono::Duration),
    SlowDown,
}

pub async fn run<F, S, I, T, Int, Fut, TFut>(fun: F, setup: S, teardown: T)
where
    F: Fn(I) -> Fut + Send + Sync + 'static,
    S: Fn() -> Int,
    T: Fn(I) -> TFut,
    I: Clone + Send + Sync + 'static,
    Int: Future<Output = I> + Send + 'static,
    Fut: Future<Output = Result<RunResult, reqwest::Error>> + Send + 'static,
    TFut: Future<Output = ()> + Send + 'static,
{
    let (tx, mut rx) = mpsc::channel(100);

    std::thread::spawn(move || {
        enable_raw_mode().unwrap();
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture).unwrap();
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).unwrap();

        let mut i = 0;
        loop {
            let data: Vec<(f64, f64)> = rx.blocking_recv().unwrap();

            terminal
                .draw(|f| {
                    let size = f.size();
                    let datasets = vec![Dataset::default()
                        .name("requests")
                        .marker(symbols::Marker::Dot)
                        .graph_type(GraphType::Line)
                        .style(Style::default().fg(Color::Cyan))
                        .data(&data)];
                    let chart = Chart::new(datasets)
                        .block(Block::default().title("Chart"))
                        .x_axis(
                            Axis::default()
                                .title(Span::styled("Time", Style::default().fg(Color::Red)))
                                .style(Style::default().fg(Color::White))
                                .bounds([0.0, data.len() as f64])
                                .labels(
                                    data.iter()
                                        .map(|(x, _)| x.to_string())
                                        .map(Span::from)
                                        .collect(),
                                ),
                        )
                        .y_axis(
                            Axis::default()
                                .title(Span::styled("Requests", Style::default().fg(Color::Red)))
                                .style(Style::default().fg(Color::White))
                                .bounds([0.0, 10000.0]),
                        );
                    f.render_widget(chart, size);
                })
                .unwrap();

            if poll(time::Duration::from_millis(5)).unwrap() {
                if let Event::Key(key) = event::read().unwrap() {
                    if key.code == KeyCode::Char('q') {
                        break;
                    }
                }
            }

            i = (i + 1) % 100;
            thread::sleep(Duration::from_millis(16));
        }

        disable_raw_mode().unwrap();
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )
        .unwrap();
        terminal.show_cursor().unwrap();
    });

    let mut target_requests_per_second: usize = 1000;
    let max_requests_in_flight: usize = 10000;

    let num_clients = 10;

    let mut request_start = Vec::new();

    let mut clients = Vec::new();
    for _ in 0..num_clients {
        clients.push(setup().await);
    }

    let mut handles = FuturesUnordered::new();

    for i in 0..target_requests_per_second {
        let client = clients[i % num_clients].clone();
        let future = fun(client);

        handles.push(tokio::spawn(async move {
            sleep(time::Duration::from_micros(
                1_000_000 * i as u64 / target_requests_per_second as u64,
            ))
            .await;
            (Instant::now(), future.await)
        }));
        request_start.push(Utc::now());
    }

    let mut last_slow_down = 0;

    let mut last_second = Utc::now().second();
    let mut num_current_second = 1;
    let mut second = 1;

    let mut data = Vec::new();

    let mut i = 0;
    while !handles.is_empty() {
        let result = handles.next().await.unwrap();
        let (start, status) = result.unwrap();

        let run_result = status.unwrap();
        if run_result == RunResult::SlowDown && i - last_slow_down > 1000 {
            target_requests_per_second -= 500;
            last_slow_down = i;

            // println!("Slow down!");
        }

        if Utc::now().second() != last_second {
            data.push((second as f64, num_current_second as f64));
            last_second = Utc::now().second();
            second += 1;
            num_current_second = 1;
            match tx.send(data.clone()).await {
                Result::Ok(_) => {}
                Result::Err(_) => {}
            };
        } else {
            num_current_second += 1;
        }

        if i % 1000 == 0 {
            target_requests_per_second += 100;
            // println!("Target requests: {}", target_requests_per_second);
        }

        target_requests_per_second = cmp::min(target_requests_per_second, max_requests_in_flight);

        let num_spawn;
        if handles.len() < (target_requests_per_second as f64 * 0.9) as usize {
            num_spawn = 2;
        } else if handles.len() < target_requests_per_second {
            num_spawn = 1;
        } else {
            num_spawn = 0;
        }

        for _ in 0..num_spawn {
            let client = clients[i % num_clients].clone();
            let future = fun(client);
            let now = Instant::now();

            handles.push(tokio::spawn(async move {
                if now - start < Duration::from_secs(1) {
                    let duration = Duration::from_secs(1) - (now - start);
                    sleep(duration).await;
                }
                (Instant::now(), future.await)
            }));
        }

        i += 1;
    }

    for c in clients.into_iter() {
        teardown(c).await;
    }
}
