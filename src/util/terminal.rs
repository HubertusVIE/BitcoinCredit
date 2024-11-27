use crate::dht::Client;
use anyhow::Result;
use log::{error, info};
use std::io::BufRead;
use tokio::sync::broadcast;

pub async fn run_terminal_client(
    mut shutdown_dht_client_receiver: broadcast::Receiver<bool>,
    mut dht_client: Client,
) -> Result<()> {
    // We need to use blocking stdin, because tokio's async stdin isn't meant for interactive
    // use-cases and will block forever on finishing the program
    let (stdin_tx, mut stdin_rx) = tokio::sync::mpsc::channel(100);
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(line) => {
                    let line = line.trim().to_string();
                    if !line.is_empty() {
                        if let Err(e) = stdin_tx.blocking_send(line) {
                            error!("Error handling stdin: {e}");
                        }
                    }
                }
                Err(e) => {
                    error!("Error reading line from stdin: {e}");
                }
            }
        }
    });

    loop {
        tokio::select! {
            line = stdin_rx.recv() => {
                if let Some(next_line) = line {
                    handle_input_line(&mut dht_client, next_line).await
                }
            },
            _ = shutdown_dht_client_receiver.recv() => {
                info!("Shutting down terminal client...");
                break;
            }
        }
    }
    Ok(())
}

async fn handle_input_line(dht_client: &mut Client, line: String) {
    let mut args = line.split(' ');
    match args.next() {
        Some("START_PROVIDING") => {
            let name: String = {
                match args.next() {
                    Some(name) => String::from(name),
                    None => {
                        error!("Expected name.");
                        return;
                    }
                }
            };
            if let Err(e) = dht_client.start_providing(name.clone()).await {
                error!("Could not start providing {name}: {e}");
            };
        }

        Some("GET_BILL") => {
            let name: String = {
                match args.next() {
                    Some(name) => String::from(name),
                    None => {
                        error!("Expected bill name.");
                        return;
                    }
                }
            };
            if let Err(e) = dht_client.get_bill(&name).await {
                error!("Get Bill failed for {name}: {e}");
            };
        }

        Some("GET_BILL_ATTACHMENT") => {
            let name: String = {
                match args.next() {
                    Some(name) => String::from(name),
                    None => {
                        error!("Expected bill name.");
                        return;
                    }
                }
            };
            let file_name: String = {
                match args.next() {
                    Some(file_name) => String::from(file_name),
                    None => {
                        error!("Expected file name.");
                        return;
                    }
                }
            };
            if let Err(e) = dht_client.get_bill_attachment(&name, &file_name).await {
                error!("Get Bill Attachment failed for {name}: {e}");
            }
        }

        Some("GET_KEY") => {
            let name: String = {
                match args.next() {
                    Some(name) => String::from(name),
                    None => {
                        error!("Expected bill name.");
                        return;
                    }
                }
            };
            if let Err(e) = dht_client.get_key(&name).await {
                error!("Get Bill Keys failed for {name}: {e}");
            }
        }

        Some("PUT_RECORD") => {
            let key = {
                match args.next() {
                    Some(key) => String::from(key),
                    None => {
                        error!("Expected key");
                        return;
                    }
                }
            };
            let value = {
                match args.next() {
                    Some(value) => String::from(value),
                    None => {
                        error!("Expected value");
                        return;
                    }
                }
            };

            if let Err(e) = dht_client.put_record(key.clone(), value.clone()).await {
                error!("Could not put record {value} to {key}: {e}");
            }
        }

        Some("SEND_MESSAGE") => {
            let topic = {
                match args.next() {
                    Some(key) => String::from(key),
                    None => {
                        error!("Expected topic");
                        return;
                    }
                }
            };
            let msg = {
                match args.next() {
                    Some(value) => String::from(value),
                    None => {
                        error!("Expected msg");
                        return;
                    }
                }
            };

            if let Err(e) = dht_client
                .send_message(msg.clone().into_bytes(), topic.clone())
                .await
            {
                error!("Could not send message {msg} to {topic}: {e}");
            }
        }

        Some("SUBSCRIBE") => {
            let topic = {
                match args.next() {
                    Some(key) => String::from(key),
                    None => {
                        error!("Expected topic");
                        return;
                    }
                }
            };

            if let Err(e) = dht_client.subscribe_to_topic(topic.clone()).await {
                error!("Could not subscribe to topic {topic}: {e}");
            }
        }

        Some("GET_RECORD") => {
            let key = {
                match args.next() {
                    Some(key) => String::from(key),
                    None => {
                        error!("Expected key");
                        return;
                    }
                }
            };
            if let Err(e) = dht_client.get_record(key.clone()).await {
                error!("Could not get record for {key}: {e}");
            }
        }

        Some("GET_PROVIDERS") => {
            let key = {
                match args.next() {
                    Some(key) => String::from(key),
                    None => {
                        error!("Expected key");
                        return;
                    }
                }
            };
            if let Err(e) = dht_client.get_providers(key.clone()).await {
                error!("Could not get providers for {key}: {e}");
            }
        }

        _ => {
            error!(
                "expected GET_BILL, GET_KEY, GET_BILL_ATTACHMENT, PUT, SEND_MESSAGE, SUBSCRIBE, GET_RECORD, PUT_RECORD or GET_PROVIDERS."
            );
        }
    }
}
