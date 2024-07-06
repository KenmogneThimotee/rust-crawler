
use regex::Regex;
use reqwest::Error;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use scraper::{Html, Selector};


enum UrlState {
    PROCESSED(Processed_url),
    UNPROCESSED(Unprocessed_url)
}

enum TaskState {
    NOT_STARTED,
    PENDING,
    PROCESSED,
    FAIL(i32)
}
struct Unprocessed_url {
    parent: Option<String>,
    url: String
}


#[derive(Debug, PartialEq)]
pub struct Processed_url {
    pub parent: Option<String>,
    pub url: String,
    pub content: String,
    pub children: Vec<String>
}

struct task_state {
    url: TaskState
}

pub async fn scrape(urls: Vec<String>, retry_attempt: i32, number_process: i32) -> impl tokio_stream::Stream<Item = Processed_url> {

    let (buffer_tx, mut buffer_rx) = mpsc::channel::<Unprocessed_url>(10);
    let (user_tx, user_rx) = mpsc::channel::<Processed_url>(10);
    let (fail_tx, mut fail_rx) = mpsc::channel::<Unprocessed_url>(10);
    let (url_tx, mut url_rx) = mpsc::channel::<Unprocessed_url>(10);
    println!("Scheduler :: 1");

    for url in urls {

        let unproc = Unprocessed_url{
            parent: None,
            url: url
        };
        buffer_tx.send(unproc).await.unwrap();
    }
    
    
    let url_tx_clone = url_tx.clone();

    tokio::spawn(async move {
        scheduler(&mut buffer_rx, user_tx, url_tx_clone, fail_tx).await;
    });


    tokio::spawn(async move {
        fail_task(&mut fail_rx, url_tx).await;
    });

    tokio::spawn( async move  {
        urls_processor(&mut url_rx, buffer_tx).await;
    });

    ReceiverStream::new(user_rx)

}

async fn scheduler(buffer_rx: &mut Receiver<Unprocessed_url>, user_tx: Sender<Processed_url>, url_tx: Sender<Unprocessed_url>, fail_tx: Sender<Unprocessed_url>){

    // println!("Scheduler ::");
    while let Some(unprocessed_url) = buffer_rx.recv().await {
        let user_tx_clone = user_tx.clone();
        let url_tx_clone = url_tx.clone();
        let fail_tx_clone = fail_tx.clone();
        println!("Scheduler ::");

        tokio::spawn(async move {
            process_url(user_tx_clone, url_tx_clone, fail_tx_clone, unprocessed_url).await;
        });
    }
}

async fn fail_task(fail_rx: &mut Receiver<Unprocessed_url>, url_tx: Sender<Unprocessed_url>){


    while let Some(unprocessed_url) = fail_rx.recv().await {
        println!("Fail task ::");

        url_tx.send(unprocessed_url).await.unwrap();
    }
}

async fn urls_processor(url_rx: &mut Receiver<Unprocessed_url>, buffer_tx: Sender<Unprocessed_url>){

    while let  Some(unprocessed_url) = url_rx.recv().await {
        println!("Url processor ::");

        buffer_tx.send(unprocessed_url).await.unwrap();
    }

}

async fn process_url(user_tx: Sender<Processed_url>, url_tx: Sender<Unprocessed_url>, fail_tx: Sender<Unprocessed_url> ,unprocessed_url: Unprocessed_url){
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
        
                    let proc = Processed_url{
                        parent: unprocessed_url.parent.clone(),
                        url: unprocessed_url.url.clone(),
                        content: body,
                        children: endpoints_extracted.clone()
                    };
        
                    println!("Fecthing OK 3 :: 78 {:?}", proc.url);
                    user_tx.send(proc).await.unwrap();

                    for endpoint in endpoints_extracted {
                        let unproc = Unprocessed_url{
                            url: endpoint,
                            parent: Some(unprocessed_url.url.clone())
                        };

                        url_tx.send(unproc).await.unwrap();

                    }
                    println!("Fecthing OK 3 :: cv");
                },
                Err(e) => {
                    println!("Fetching not ok 2 :: {:?}",e);
                    fail_tx.send(unprocessed_url).await.unwrap();
                }
            }
        },
        Err(e) => {
            println!("{:?}",&e);
            println!("Fecthing Not OK::");

            fail_tx.send(unprocessed_url).await.unwrap();
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