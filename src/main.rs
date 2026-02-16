use std::fs::{create_dir_all, File};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use clap::{Parser, ArgAction};

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Output directory for the CSV file
    #[arg(long, short)]
    directory: String,

    /// Default baud rate (overridden per-port if specified)
    #[arg(long, default_value_t = 19200)]
    default_baud: u32,

    /// Serial port(s) to read from, format: <port>[,<baud>]
    #[arg(long = "port", short = 'p', action = ArgAction::Append)]
    ports: Vec<String>,
}

fn parse_port_arg(arg: &str, default_baud: u32) -> (String, u32) {
    let mut parts = arg.splitn(2, ',');
    let port = parts.next().unwrap().to_string();
    let baud = parts
        .next()
        .map(|b| b.parse().unwrap_or(default_baud))
        .unwrap_or(default_baud);
    (port, baud)
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    if args.ports.is_empty() {
        eprintln!("At least one --port argument is required.");
        std::process::exit(1);
    }
    if args.ports.len() > 8 {
        eprintln!("Maximum 8 ports supported");
        std::process::exit(1);
    }

    let directory = Path::new(&args.directory);
    create_dir_all(directory)?;

    let file_path = directory.join(format!("{}.csv", timestamp()));
    println!("Recording to {:?} (Ctrl+C to stop)", file_path);

    let mut file = File::create(file_path)?;

    let (tx, rx) = mpsc::channel::<(String, String)>();

    let port_settings: Vec<(String, u32)> = args
        .ports
        .iter()
        .map(|p| parse_port_arg(p, args.default_baud))
        .collect();

    for (port_name, baud) in port_settings {
        let tx = tx.clone();
        let port_name = port_name.clone();

        thread::spawn(move || {
            let port = serialport::new(&port_name, baud)
                .open()
                .expect("Failed to open serial port");

            let reader = BufReader::new(port);

            for line in reader.lines() {
                if let Ok(data) = line {
                    let _ = tx.send((port_name.clone(), data));
                }
            }
        });
    }

    drop(tx); // close original sender

    for (port_name, line) in rx {
        writeln!(file, "{},{},{}", timestamp(), port_name, line)?;
    }

    Ok(())
}
