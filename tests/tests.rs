use stream_crawler;
use tokio_stream::StreamExt;

#[cfg(test)]
mod tests {
    use stream_crawler::extract_urls_from_a_tags;

    use super::*;

    #[tokio::test]
    async fn test_scrape() {
        let urls = vec![
            String::from("https://www.google.com"),
            String::from("https://www.twitter.com"),
        ];

        let mut result_stream = stream_crawler::scrape(urls, 3, 5).await;

        let mut results = vec![];
        while let Some(data) = result_stream.next().await {
            results.push(data);
            break;
        }

        // This is a simple test and might need to be adjusted based on actual results.
        assert!(!results.is_empty());
    }

    #[test]
    fn test_extract_urls_from_a_tags() {
        let html = r#"
        <html>
            <body>
                <a href="https://www.example.com/page1">Page 1</a>
                <a href="https://www.example.com/page2">Page 2</a>
            </body>
        </html>
        "#;

        let urls = extract_urls_from_a_tags(html);
        assert_eq!(urls, vec![
            "https://www.example.com/page1",
            "https://www.example.com/page2"
        ]);
    }
}
