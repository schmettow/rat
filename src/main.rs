use std::fs::{create_dir_all, File};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

struct Args {
    directory: String,
    default_baud: u32,
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

fn print_usage() {
    eprintln!("Usage: rat [OPTIONS]");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("  -d, --directory <DIR>        Output directory for the CSV file (required)");
    eprintln!("  -b, --default-baud <BAUD>   Default baud rate (default: 19200)");
    eprintln!("  -p, --port <PORT[,BAUD]>    Serial port to read from. Can be specified multiple times");
    eprintln!("                               Format: /dev/ttyUSB0 or /dev/ttyUSB0,9600");
    eprintln!("  -h, --help                   Print this help message");
}

fn parse_args() -> Result<Args, String> {
    let args: Vec<String> = std::env::args().collect();

    let mut directory = String::new();
    let mut default_baud = 19200u32;
    let mut ports = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            "-d" | "--directory" => {
                i += 1;
                if i >= args.len() {
                    return Err("--directory requires an argument".to_string());
                }
                directory = args[i].clone();
            }
            "-b" | "--default-baud" => {
                i += 1;
                if i >= args.len() {
                    return Err("--default-baud requires an argument".to_string());
                }
                default_baud = args[i].parse()
                    .map_err(|_| format!("Invalid baud rate: {}", args[i]))?;
            }
            "-p" | "--port" => {
                i += 1;
                if i >= args.len() {
                    return Err("--port requires an argument".to_string());
                }
                ports.push(args[i].clone());
            }
            arg => {
                return Err(format!("Unknown argument: {}", arg));
            }
        }
        i += 1;
    }

    if directory.is_empty() {
        return Err("--directory is required".to_string());
    }

    if ports.is_empty() {
        return Err("At least one --port argument is required".to_string());
    }

    if ports.len() > 8 {
        return Err("Maximum 8 ports supported".to_string());
    }

    Ok(Args {
        directory,
        default_baud,
        ports,
    })
}

fn main() -> std::io::Result<()> {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!();
            print_usage();
            std::process::exit(1);
        }
    };

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
            let port = match serialport::new(&port_name, baud).open() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Failed to open serial port {}: {}", port_name, e);
                    return;
                }
            };

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
