use std::{collections::HashMap, sync::Arc};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio_stream::wrappers::ReceiverStream;
use scraper::{Html, Selector};

/// Represents the state of a task.
enum TaskState {
    NotStarted(usize),
    PENDING(usize),
    PROCESSED(usize),
    FAILED(usize),
}

/// Represents an unprocessed URL.
struct UnprocessedUrl {
    parent: Option<String>,
    url: String,
}

/// Represents a processed URL.
///
/// # Examples
///
/// ```
/// use stream_crawler::ProcessedUrl;
///
/// let processed_url = ProcessedUrl {
///     parent: Some(String::from("https://www.example.com")),
///     url: String::from("https://www.example.com/page"),
///     content: String::from("<html>...</html>"),
///     children: vec![String::from("https://www.example.com/page/1")],
/// };
///
/// assert_eq!(processed_url.url, "https://www.example.com/page");
/// ```
#[derive(Debug, PartialEq)]
pub struct ProcessedUrl {
    pub parent: Option<String>,
    pub url: String,
    pub content: String,
    pub children: Vec<String>,
}

/// Scrapes the given URLs, returning a stream of processed URLs.
///
/// # Examples
///
/// ```
/// use stream_crawler::scrape;
/// use tokio_stream::StreamExt;
///
/// #[tokio::main]
/// async fn main() {
///     let urls = vec![
///         String::from("https://www.google.com"),
///         String::from("https://www.twitter.com"),
///     ];
///
///     let mut result_stream = scrape(urls, 3, 5, 9).await;
///     let mut results = vec![];
///
///     while let Some(data) = result_stream.next().await {
///         results.push(data);
///         break;
///     }
///     assert!(!results.is_empty());
/// }
/// ```
pub async fn scrape(urls: Vec<String>, retry_attempt: usize, number_process: usize, channel_size: usize) -> impl tokio_stream::Stream<Item = ProcessedUrl> {
    let (buffer_tx, mut buffer_rx) = mpsc::channel::<UnprocessedUrl>(channel_size);
    let (user_tx, user_rx) = mpsc::channel::<ProcessedUrl>(channel_size);
    let (fail_tx, mut fail_rx) = mpsc::channel::<(UnprocessedUrl, reqwest::Error)>(channel_size);
    let (url_tx, mut url_rx) = mpsc::channel::<UnprocessedUrl>(channel_size);
    
    let task_state: Arc<tokio::sync::Mutex<HashMap<String, TaskState>>> = Arc::new(tokio::sync::Mutex::new(HashMap::new()));

    for url in urls {
        let unproc = UnprocessedUrl {
            parent: None,
            url: url.clone(),
        };
        task_state.clone().lock_owned().await.insert(url.clone(), TaskState::NotStarted(0));
        buffer_tx.send(unproc).await.unwrap();
    }
    
    let url_tx_clone = url_tx.clone();
    let cloned_task_state = task_state.clone();
    tokio::spawn(async move {
        scheduler(&mut buffer_rx, user_tx, url_tx_clone, fail_tx, number_process, cloned_task_state).await;
    });

    let cloned_task_state = task_state.clone();
    tokio::spawn(async move {
        fail_task(&mut fail_rx, url_tx, retry_attempt, cloned_task_state).await;
    });

    let cloned_task_state = task_state.clone();
    tokio::spawn(async move {
        urls_processor(&mut url_rx, buffer_tx, cloned_task_state).await;
    });

    ReceiverStream::new(user_rx)
}

async fn scheduler(buffer_rx: &mut Receiver<UnprocessedUrl>, user_tx: Sender<ProcessedUrl>, url_tx: Sender<UnprocessedUrl>, fail_tx: Sender<(UnprocessedUrl, reqwest::Error)>, number_process: usize, task_state: Arc<tokio::sync::Mutex<HashMap<String, TaskState>>>) {
    let semaphore = Arc::new(tokio::sync::Semaphore::new(number_process));
    while let Some(unprocessed_url) = buffer_rx.recv().await {
        let user_tx_clone = user_tx.clone();
        let url_tx_clone = url_tx.clone();
        let fail_tx_clone = fail_tx.clone();

        let mut state = task_state.clone().lock_owned().await;
        let key_value = state.get(&unprocessed_url.url);
        match key_value {
            Some(TaskState::FAILED(value)) => {
                let val = value.clone();
                state.insert(unprocessed_url.url.clone(), TaskState::PENDING(val));
            },
            Some(TaskState::NotStarted(value)) => {
                let val = value.clone();
                state.insert(unprocessed_url.url.clone(), TaskState::PENDING(val));
            },
            None => {
                state.insert(unprocessed_url.url.clone(), TaskState::PENDING(0));
            },
            _ => continue
        }
        let sem = semaphore.clone();
        tokio::spawn(async move {
            let permit = sem.acquire_owned().await.unwrap();
            process_url(user_tx_clone, url_tx_clone, fail_tx_clone, unprocessed_url).await;
            drop(permit);
        });
    }
}

async fn fail_task(fail_rx: &mut Receiver<(UnprocessedUrl, reqwest::Error)>, url_tx: Sender<UnprocessedUrl>, retry_attempt: usize, task_state: Arc<tokio::sync::Mutex<HashMap<String, TaskState>>>) {
    while let Some(unprocessed_url) = fail_rx.recv().await {


        let mut state = task_state.clone().lock_owned().await;
        let key_value = state.get(&unprocessed_url.0.url);
        match key_value {
            Some(TaskState::PENDING(value)) => {
                if unprocessed_url.1.is_connect() && retry_attempt > value.clone() {

                    let val = value.clone();
                    state.insert(unprocessed_url.0.url.clone(), TaskState::FAILED(val + 1));
                    url_tx.send(unprocessed_url.0).await.unwrap();
                }
            },
            None => continue,
            _ => continue
        }
    }
}

async fn urls_processor(url_rx: &mut Receiver<UnprocessedUrl>, buffer_tx: Sender<UnprocessedUrl>, task_state: Arc<tokio::sync::Mutex<HashMap<String, TaskState>>>) {
    while let Some(unprocessed_url) = url_rx.recv().await {

        let mut state = task_state.clone().lock_owned().await;
        let key_value = state.get(&unprocessed_url.url);
        match key_value {
            Some(TaskState::FAILED(value)) => {
                println!("url tx {:?}", value);
                let val = value.clone();
                state.insert(unprocessed_url.url.clone(), TaskState::FAILED(val));
                buffer_tx.send(unprocessed_url).await.unwrap();
            },
            Some(TaskState::PENDING(value)) => {
                let val = value.clone();
                state.insert(unprocessed_url.url.clone(), TaskState::PROCESSED(val));
            },
            None => {
                state.insert(unprocessed_url.url.clone(), TaskState::NotStarted(0));
                buffer_tx.send(unprocessed_url).await.unwrap();
            },
            _ => continue
        }
    }
}

async fn process_url(user_tx: Sender<ProcessedUrl>, url_tx: Sender<UnprocessedUrl>, fail_tx: Sender<(UnprocessedUrl, reqwest::Error)>, unprocessed_url: UnprocessedUrl) {
    println!("Task :: {}", unprocessed_url.url.clone());
    let resp = reqwest::get(unprocessed_url.url.clone()).await;

    match resp {
        Ok(response) => {
            let text = response.text().await;
            match text {
                Ok(body) => {
                    let urls_extracted = extract_urls_from_a_tags(&body);
                    let mut endpoints_extracted: Vec<String> = [].to_vec();
                    for endpoint in &urls_extracted {
                        if endpoint.starts_with('/') {
                            endpoints_extracted.push(unprocessed_url.url.clone() + endpoint)
                        } else {
                            endpoints_extracted.push(endpoint.clone());
                        }
                    }

                    let proc = ProcessedUrl {
                        parent: unprocessed_url.parent.clone(),
                        url: unprocessed_url.url.clone(),
                        content: body,
                        children: endpoints_extracted.clone(),
                    };

                    user_tx.send(proc).await.unwrap();

                    for endpoint in endpoints_extracted {
                        let unproc = UnprocessedUrl {
                            url: endpoint,
                            parent: Some(unprocessed_url.url.clone()),
                        };
                        url_tx.send(unproc).await.unwrap();
                    }
                },
                Err(e) => {
                    fail_tx.send((unprocessed_url, e)).await.unwrap();
                }
            }
        },
        Err(e) => {
            fail_tx.send((unprocessed_url, e)).await.unwrap();
        }
    }
}

/// Extracts URLs from the <a> tags in the given HTML string.
///
/// # Examples
///
/// ```
/// use stream_crawler::extract_urls_from_a_tags;
///
/// let html = r#"
/// <html>
///     <body>
///         <a href="https://www.example.com/page1">Page 1</a>
///         <a href="https://www.example.com/page2">Page 2</a>
///     </body>
/// </html>
/// "#;
///
/// let urls = extract_urls_from_a_tags(html);
/// assert_eq!(urls, vec![
///     "https://www.example.com/page1",
///     "https://www.example.com/page2"
/// ]);
/// ```
pub fn extract_urls_from_a_tags(html: &str) -> Vec<String> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("a").unwrap();
    let data = document
        .select(&selector)
        .filter_map(|element| element.value().attr("href"))
        .map(|href| href.to_string())
        .collect();
    data
}
