#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::io::{BufRead, Write};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use owo_colors::OwoColorize;
use tracing_subscriber::EnvFilter;

use mini_rag::{DB_PATH, db, embed, rag};

const SPINNER_FRAMES: &[char] = &[
    '\u{280B}', '\u{2819}', '\u{2839}', '\u{2838}', '\u{283C}', '\u{2834}', '\u{2826}', '\u{2827}',
    '\u{2807}', '\u{280F}',
];

fn start_spinner(message: &str) -> mpsc::Sender<()> {
    let (tx, rx) = mpsc::channel();
    let msg = message.to_owned();
    std::thread::spawn(move || {
        let mut i = 0;
        #[allow(clippy::indexing_slicing)]
        loop {
            print!(
                "\r\x1b[2K{} {msg}",
                SPINNER_FRAMES[i % SPINNER_FRAMES.len()].yellow()
            );
            std::io::stdout().flush().ok();
            if rx.recv_timeout(Duration::from_millis(80)).is_ok() {
                break;
            }
            i += 1;
        }
        print!("\r\x1b[2K");
        std::io::stdout().flush().ok();
    });
    tx
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("warn".parse().expect("valid directive")),
        )
        .init();

    let conn = db::init_db(Path::new(DB_PATH)).await?;
    let client = mini_rag::ollama_client()?;
    let embedding_model = embed::embedding_model(&client);

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    println!(
        "{} — ask questions about your ingested documents",
        "rag-chat".bold().cyan()
    );
    println!(
        "Type {} or {} to leave.\n",
        "exit".dimmed(),
        "quit".dimmed()
    );

    loop {
        print!("{} ", ">".bold().green());
        stdout.flush()?;

        let mut line = String::new();
        let bytes_read = stdin.lock().read_line(&mut line)?;
        if bytes_read == 0 {
            break;
        }

        let question = line.trim();
        if question.is_empty() || question == "exit" || question == "quit" {
            break;
        }

        let spinner = start_spinner("Searching and thinking...");
        let result = rag::query(&client, &embedding_model, &conn, question).await;
        spinner.send(()).ok();
        std::thread::sleep(Duration::from_millis(10));

        match result {
            Ok(response) => println!("\n{response}"),
            Err(e) => eprintln!("\n{} {e}\n", "Error:".red().bold()),
        }
    }

    Ok(())
}
