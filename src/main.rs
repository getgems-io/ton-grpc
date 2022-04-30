use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use serde_json::json;
use crate::tonlib::Client;

mod tonlib;

fn main() {
    let client = Arc::new(Client::new());

    client.send(json!({
        "@type": "setLogVerbosityLevel",
        "new_verbosity_level": 0
    }).to_string().as_str());

    let inner = Arc::clone(&client);
    thread::spawn(move || {
        for _ in 1..1000 {
            let query = json!({
                "@type": "blocks.getMasterchainInfo"
            }).to_string();

            inner.send(query.as_str());
        }
    });

    let mut counter = 0;
    let start = Instant::now();
    while counter < 1000 {
        let resp = client.receive(Duration::from_secs(5));
        println!("{:#?}", resp);

        counter += 1;

        println!("{}", counter);

    }

    println!("{:?}", Instant::now() - start)
}
