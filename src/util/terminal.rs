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
        Some("PUT") => {
            let name: String = {
                match args.next() {
                    Some(name) => String::from(name),
                    None => {
                        error!("Expected name.");
                        return;
                    }
                }
            };
            dht_client.put(&name).await;
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
            dht_client.get_bill(name).await;
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
            if let Err(e) = dht_client.get_bill_attachment(name, file_name).await {
                error!("Get Bill Attachment failed: {e}");
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
            dht_client.get_key(name).await;
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

            dht_client.put_record(key, value).await;
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

            dht_client.send_message(msg.into_bytes(), topic).await;
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

            dht_client.subscribe_to_topic(topic).await;
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
            dht_client.get_record(key).await;
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
            dht_client.get_providers(key).await;
        }

        _ => {
            error!(
                "expected GET_BILL, GET_KEY, GET_BILL_ATTACHMENT, PUT, SEND_MESSAGE, SUBSCRIBE, GET_RECORD, PUT_RECORD or GET_PROVIDERS."
            );
        }
    }
}
