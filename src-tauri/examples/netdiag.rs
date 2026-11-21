use std::error::Error;
#[tokio::main]
async fn main() {
    let key = std::env::var("XX_KEY").unwrap_or_else(|_| "sk-xc-diagnostic-not-a-key".into());
    for (name, client) in [
        ("system", reqwest::Client::builder().timeout(std::time::Duration::from_secs(15)).redirect(reqwest::redirect::Policy::none()).build().unwrap()),
        ("no-proxy", reqwest::Client::builder().no_proxy().timeout(std::time::Duration::from_secs(15)).redirect(reqwest::redirect::Policy::none()).build().unwrap()),
    ] {
        for url in ["https://api.xxchun.com/v1/models", "https://xxchun.com/", "https://www.baidu.com/"] {
            match client.get(url).bearer_auth(&key).header("Accept", "application/json").send().await {
                Ok(r) => println!("[{name}] {url} -> HTTP {}", r.status()),
                Err(e) => {
                    let mut s = format!("{e}");
                    let mut src = e.source();
                    while let Some(x) = src { s.push_str(&format!(" | {x}")); src = x.source(); }
                    println!("[{name}] {url} -> ERROR {}", s.replace(&key, "***"));
                }
            }
        }
    }
}
