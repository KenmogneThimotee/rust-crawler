
use std::{borrow::BorrowMut, collections::HashMap, sync::Arc};

use regex::Regex;
use reqwest::Error;
use tokio::{runtime, sync::mpsc::{self, Receiver, Sender}};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use scraper::{Html, Selector};


enum UrlState {
    PROCESSED(ProcessedUrl),
    UNPROCESSED(UnprocessedUrl)
}

enum TaskState {
    NOT_STARTED(i32),
    PENDING(i32),
    PROCESSED(i32),
    FAIL(i32)
}
struct UnprocessedUrl {
    parent: Option<String>,
    url: String
}


#[derive(Debug, PartialEq)]
pub struct ProcessedUrl {
    pub parent: Option<String>,
    pub url: String,
    pub content: String,
    pub children: Vec<String>
}



pub async fn scrape(urls: Vec<String>, retry_attempt: i32, number_process: usize) -> impl tokio_stream::Stream<Item = ProcessedUrl> {

    let (buffer_tx, mut buffer_rx) = mpsc::channel::<UnprocessedUrl>(10);
    let (user_tx, user_rx) = mpsc::channel::<ProcessedUrl>(10);
    let (fail_tx, mut fail_rx) = mpsc::channel::<(UnprocessedUrl, reqwest::Error)>(10);
    let (url_tx, mut url_rx) = mpsc::channel::<UnprocessedUrl>(10);
    println!("Scheduler :: 1");
    let task_state: Arc<tokio::sync::Mutex<HashMap<String, TaskState>>> = Arc::new(tokio::sync::Mutex::new(HashMap::new()));

    for url in urls {

        let unproc = UnprocessedUrl{
            parent: None,
            url: url.clone()
        };

        task_state.clone().lock_owned().await.insert(url.clone(), TaskState::NOT_STARTED(0));

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
    tokio::spawn( async move  {
        urls_processor(&mut url_rx, buffer_tx, cloned_task_state).await;
    });

    ReceiverStream::new(user_rx)

}

async fn scheduler(buffer_rx: &mut Receiver<UnprocessedUrl>, user_tx: Sender<ProcessedUrl>, url_tx: Sender<UnprocessedUrl>, fail_tx: Sender<(UnprocessedUrl, reqwest::Error)>, number_process: usize, task_state: Arc<tokio::sync::Mutex<HashMap<String, TaskState>>>){

    let semaphore = Arc::new(tokio::sync::Semaphore::new(number_process));
    // println!("Scheduler ::");
    while let Some(unprocessed_url) = buffer_rx.recv().await {
        let user_tx_clone = user_tx.clone();
        let url_tx_clone = url_tx.clone();
        let fail_tx_clone = fail_tx.clone();
        println!("Scheduler ::");
        let mut state = task_state.clone().lock_owned().await;
        let key_value = state.get(&unprocessed_url.url);
        match key_value {
            Some(TaskState::FAIL(value)) => {
                let val = value.clone();
                state.insert(unprocessed_url.url.clone(), TaskState::PENDING(val));
            },
            Some(TaskState::NOT_STARTED(value)) => {
                let val = value.clone();
                state.insert(unprocessed_url.url.clone(), TaskState::PENDING(val));
            },
            Some(TaskState::PENDING(_)) => {
                continue;
            },
            Some(TaskState::PROCESSED(_)) => {
                continue;
            },
            None => {
                state.insert(unprocessed_url.url.clone(), TaskState::PENDING(0));
            }
        };
        let sem = semaphore.clone();
        tokio::spawn(async move {

            let permit = sem.acquire_owned().await.unwrap() ;// .acquire().await.unwrap();

            process_url(user_tx_clone, url_tx_clone, fail_tx_clone, unprocessed_url).await;
            drop(permit);
        });
    }
}

async fn fail_task(fail_rx: &mut Receiver<(UnprocessedUrl, reqwest::Error)>, url_tx: Sender<UnprocessedUrl>, retry_attempt: i32, task_state: Arc<tokio::sync::Mutex<HashMap<String, TaskState>>>){


    while let Some(unprocessed_url) = fail_rx.recv().await {
        println!("Fail task ::");
        if unprocessed_url.1.is_connect(){
            println!("retry");
        }

        let mut state = task_state.clone().lock_owned().await;
        let key_value = state.get(&unprocessed_url.0.url);
        match key_value {
            Some(TaskState::FAIL(_)) => {
                continue;
            },
            Some(TaskState::NOT_STARTED(_)) => {
                continue;
            },
            Some(TaskState::PENDING(value)) => {
                if unprocessed_url.1.is_connect()  && retry_attempt > value.clone(){
                    println!("retry 1");
                    let val = value.clone();
                    state.insert(unprocessed_url.0.url.clone(), TaskState::FAIL(val+1));
                    url_tx.send(unprocessed_url.0).await.unwrap();
                }

            },
            Some(TaskState::PROCESSED(_)) => {
                continue;
            },
            None => {continue;}
        };

    }
}

async fn urls_processor(url_rx: &mut Receiver<UnprocessedUrl>, buffer_tx: Sender<UnprocessedUrl>, task_state: Arc<tokio::sync::Mutex<HashMap<String, TaskState>>>){



    while let  Some(unprocessed_url) = url_rx.recv().await {
        println!("Url processor ::");

        let mut state = task_state.clone().lock_owned().await;
        let key_value = state.get(&unprocessed_url.url);
        match key_value {
            Some(TaskState::FAIL(value)) => {
                println!("url tx {:?}",value);
                let val = value.clone();
                state.insert(unprocessed_url.url.clone(), TaskState::FAIL(val));
                buffer_tx.send(unprocessed_url).await.unwrap();
            },
            Some(TaskState::NOT_STARTED(_)) => {
                continue;
            },
            Some(TaskState::PENDING(value)) => {
                let val = value.clone();
                state.insert(unprocessed_url.url.clone(), TaskState::PROCESSED(val));
            },
            Some(TaskState::PROCESSED(_)) => {
                continue;
            },
            None => {
                state.insert(unprocessed_url.url.clone(), TaskState::NOT_STARTED(0));
                buffer_tx.send(unprocessed_url).await.unwrap();
            }
        };

    }

}

async fn process_url(user_tx: Sender<ProcessedUrl>, url_tx: Sender<UnprocessedUrl>, fail_tx: Sender<(UnprocessedUrl, reqwest::Error)> ,unprocessed_url: UnprocessedUrl){
// Implementation to process the URL and return a Processed_url
// ...
    println!("Task :: {}", unprocessed_url.url.clone() );

    let resp = reqwest::get(unprocessed_url.url.clone()).await;
    println!("Fecthing ::");

    match resp {
        Ok(response) => {
            let text = response.text().await;

            match text {
                Ok(body) => {
                    let urls_extracted = extract_urls_from_a_tags(&body);
                    let mut endpoints_extracted: Vec<String>  = [].to_vec(); //= extract_endpoints(&body);
        
                    for endpoint in &urls_extracted {
                        if endpoint.starts_with('/'){
                            endpoints_extracted.push(unprocessed_url.url.clone() + &endpoint)
                        }else{
                            endpoints_extracted.push(endpoint.clone());
                        }
                    } 
        
                    // for endpoint in endpoints_extracted {
                    //     urls_extracted.push(unprocessed_url.url.clone() + &endpoint)
                    // }
        
                    println!("Fecthing OK 3 :: {:?}", endpoints_extracted);
        
                    let proc = ProcessedUrl{
                        parent: unprocessed_url.parent.clone(),
                        url: unprocessed_url.url.clone(),
                        content: body,
                        children: endpoints_extracted.clone()
                    };
        
                    println!("Fecthing OK 3 :: 78 {:?}", proc.url);
                    user_tx.send(proc).await.unwrap();

                    for endpoint in endpoints_extracted {
                        let unproc = UnprocessedUrl{
                            url: endpoint,
                            parent: Some(unprocessed_url.url.clone())
                        };

                        url_tx.send(unproc).await.unwrap();

                    }
                    println!("Fecthing OK 3 :: cv");
                },
                Err(e) => {
                    fail_tx.send((unprocessed_url, e)).await.unwrap();
                }
            }
        },
        Err(e) => {
            println!("Fecthing Not OK::");

            fail_tx.send((unprocessed_url, e)).await.unwrap();
        }
    }

}

pub fn extract_urls(text: &str) -> Vec<String> {
    let url_pattern = r"https?://[^\s/$.?#].[^\s]*";
    let re = Regex::new(url_pattern).unwrap();
    re.find_iter(text)
        .map(|mat| mat.as_str().to_string())
        .collect()
}


fn extract_endpoints(body: &str) -> Vec<String> {
    let re = Regex::new(r"/[^/]+").unwrap();
    let mut endpoints = Vec::new();

    for cap in re.captures_iter(body) {
        endpoints.push(cap[0].to_string());
    }

    endpoints
}

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