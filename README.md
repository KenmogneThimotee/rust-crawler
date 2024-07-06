# Stream crawler

`stream-scraper` is a Rust crate that provides an asynchronous web crawling utility. It processes URLs, extracts content and child URLs, and handles retry attempts for failed requests. It uses the `tokio` runtime for asynchronous operations and the `reqwest` library for HTTP requests.

## Features

- Asynchronous crawling using `tokio`
- Extracts URLs from `<a>` tags in HTML
- Retries failed requests up to a specified number of attempts
- Limits the number of concurrent requests using a semaphore

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
stream_crawler = "0.1.0"
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.11", features = ["json"] }
scraper = "0.12"
```


## Usage

```rust
use stream_crawler::scrape;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() {
    let urls = vec![
        String::from("https://www.google.com"),
        String::from("https://www.twitter.com"),
    ];

    let mut result_stream = scrape(urls, 3, 5).await;

    while let Some(data) = result_stream.next().await {
        println!("Processed URL: {:?}", data);
    }
}

```


### Functionality

1. **`scrape` function** :

* Takes a vector of URLs, a retry attempt limit, and a maximum number of concurrent processes.
* Returns a stream of `ProcessedUrl` structures.

1. **`ProcessedUrl` structure** :

* Contains the original URL, the parent URL (if any), the HTML content of the page, and a list of child URLs extracted from `<a>` tags.

### Example

This example demonstrates how to use the `scrape` function to process a list of URLs.

```rust
use stream_crawler::scrape;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() {
    let urls = vec![
        String::from("https://www.google.com"),
        String::from("https://www.twitter.com"),
    ];

    let mut result_stream = scrape(urls, 3, 5).await;

    while let Some(data) = result_stream.next().await {
        println!("Processed URL: {:?}", data);
    }
}

```


## Documentation

Refer to the inline documentation for detailed usage and examples.

### ` ProcessedUrl`

```rust
#[derive(Debug, PartialEq)]
pub struct ProcessedUrl {
    pub parent: Option<String>,
    pub url: String,
    pub content: String,
    pub children: Vec<String>,
}

```


## Contributing

Contributions are welcome! Please open an issue or submit a pull request.

## License

This project is licensed under the MIT License.

This `README.md` provides an overview of the crate, its features, installation instructions, and usage examples. You can customize it further based on your specific requirements.
