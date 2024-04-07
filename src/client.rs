use std::collections::HashSet;
use std::io::{self};
use std::net::UdpSocket;
use std::str;

fn main() -> io::Result<()> {
    // println!("Enter the server address (e.g., 127.0.0.1:8083):");
    // let mut server_address = String::new();
    // io::stdin().read_line(&mut server_address)?;
    // let server_address = server_address.trim();
    // println!("server_address: {}", server_address);

    let server_address = String::from("127.0.0.1:8083");
    let client = UdpSocket::bind("0.0.0.0:0")?;

    client.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;

    let mut received_chunks = HashSet::new();
    let mut last_chunk_received = false;
    let mut file_data: Vec<u8> = Vec::new();

    loop {
        if !last_chunk_received {
            println!("Enter the filename to request or 'RESEND' to request missing chunks:");
            let mut filename_or_command = String::new();
            io::stdin().read_line(&mut filename_or_command)?;
            let filename_or_command = filename_or_command.trim();

            let request = if filename_or_command.to_uppercase() == "RESEND" {
                format!("RESEND") // This would include logic to specify which chunks are missing
            } else {
                format!("@{}:{}/{}", server_address, server_address, filename_or_command)
            };
            client.send_to(request.as_bytes(), server_address.clone())?;
        }

        let mut buf = [0; 1500];
        match client.recv_from(&mut buf) {
            Ok((amt, src)) => {
                if src.to_string() == server_address {
                    let data = &buf[..amt];
                    if data.starts_with(b"EOF") {
                        println!("End of file reached.");
                        last_chunk_received = true;
                        continue;
                    } else if data.starts_with(b"ERR:") {
                        println!("Error: {}", str::from_utf8(&data[4..]).unwrap_or("Unknown error"));
                        return Ok(());
                    }

                    let chunk_number = parse_chunk_number(data); // Implement this based on your protocol
                    if received_chunks.contains(&chunk_number) {
                        continue; // Skip duplicate chunks
                    }

                    println!("Received chunk {}. Keep it? (Y/n):", chunk_number);
                    let mut decision = String::new();
                    io::stdin().read_line(&mut decision)?;
                    if decision.trim().eq_ignore_ascii_case("n") {
                        println!("Discarding chunk {}", chunk_number);
                        continue; // Simulate the loss of this chunk
                    }

                    received_chunks.insert(chunk_number);
                    file_data.extend_from_slice(&data); // Assume direct data addition for simplicity
                }
            },
            Err(e) => {
                println!("Timeout or error: {}", e);
                if last_chunk_received {
                    break;
                }
            },
        }
    }

    // Placeholder for file content display or processing
    println!("File received. Total chunks: {}", received_chunks.len());
    println!("Received file content: {}", String::from_utf8_lossy(&file_data));

    Ok(())
}

// Dummy implementation: extract chunk number from the data.
// In a real scenario, this should parse the actual structure of your data packet to find the chunk number.
fn parse_chunk_number(data: &[u8]) -> i32 {
    // This is a placeholder. You'll need to replace this with logic that parses your chunk's metadata.
    0
}
