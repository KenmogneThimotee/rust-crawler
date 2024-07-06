use web_scraper;
use tokio_stream::StreamExt;



#[tokio::test]
async fn test_add_integration() {
    println!("Test qdd");

    let mut urls: Vec<String> = Vec::new();
    urls.push(String::from("https://www.google.com"));
    urls.push(String::from("https://www.twitter.com"));
    let mut result = web_scraper::scrape(urls, 9, 9).await;

    let mut count = 1;
    while let Some(data) = &result.next().await {
        // let d = data;
        println!("{}",count);
        println!("Processed url {:?}", data.url);
        count += 1;

    }
    assert!(true)
}