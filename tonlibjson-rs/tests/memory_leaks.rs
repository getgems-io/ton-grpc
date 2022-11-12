use tonlibjson_rs::Client;

#[test]
fn test_mem() {
    let memory_before = procinfo::pid::statm_self().unwrap().resident;

    {
        Client::set_verbosity_level(0);

        let mut clients = vec![];
        for _ in 1..10000 {
            let client = Client::new();
            clients.push(client);

            if clients.len() > 32 {
                clients.pop();
            }
        }

        drop(clients);
    }

    let memory_after = procinfo::pid::statm_self().unwrap().resident;

    assert!(
        memory_after < memory_before + 4096,
        "Memory usage at server start is {}KB, memory usage after is {}KB. Diff is {}KB",
        memory_before,
        memory_after,
        memory_after - memory_before
    );
}
