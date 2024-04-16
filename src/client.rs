use std::collections::{HashMap, HashSet};
use std::io::{self, stdin, Write};
use std::net::UdpSocket;
use std::{env, fs, str};
use std::time::Duration;
use std::fs::File;

fn main() -> io::Result<()> {
    let server_addr = "127.0.0.1:8083".to_string();
    let filename = "medium.txt".to_string();
    
    println!("Would you like to simulate packet loss? (yes/no)");
    let simulate_loss = read_input()?.to_lowercase() == "yes";
    let mut loss_packets = HashSet::new();

    if simulate_loss {
        println!("Enter the sequence numbers of packets to simulate loss (separated by commas, no spaces):");
        let loss_input = read_input()?;
        loss_packets = loss_input.split(',')
            .filter_map(|num| num.parse::<u32>().ok())
            .collect();
    }
    
    let message = format!("GET /{}", filename);
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    send_request(&socket, &server_addr, &message)?;

    let mut packets: HashMap<u32, Vec<u8>> = HashMap::new();
    let mut received_seq_numbers = HashSet::new();
    let mut expected_num_packets;

    loop {
        let (new_data, new_seqs) = receive_response(&socket, simulate_loss, &mut loss_packets)?;
        for (data, &seq_number) in new_data.iter().zip(new_seqs.iter()) {
            packets.insert(seq_number, data.clone());  // Use actual sequence number for key
            received_seq_numbers.insert(seq_number);   // Store actual sequence number
            println!("Seq number received: {}", seq_number);
        }

        expected_num_packets = calculate_expected_number_of_packets(&received_seq_numbers);
        let missing_packets = identify_missing_packets(&received_seq_numbers, expected_num_packets);

        if missing_packets.is_empty() {
            println!("All packets received. Proceeding to file writing.");
            break;
        } else {
            println!("Missing packets detected: {:?}", missing_packets);
            request_retransmission(&socket, &server_addr, &missing_packets)?;
        }
    }

    // Now that all packets are confirmed received, write them to the file
    match write_to_file(&filename, &packets, expected_num_packets) {
        Ok(_) => println!("File '{}' successfully saved.", filename),
        Err(err) => println!("Error saving file: {}", err)
    }

    Ok(())
}


fn read_input() -> io::Result<String> {
    let mut input = String::new();
    stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn send_request(socket: &UdpSocket, server_addr: &str, message: &str) -> io::Result<()> {
	socket.send_to(message.as_bytes(), server_addr)?;
	Ok(())
}

fn receive_response(socket: &UdpSocket, simulate_loss: bool, loss_packets: &mut HashSet<u32>) -> io::Result<(Vec<Vec<u8>>, HashSet<u32>)> {
    let mut packets = Vec::new();
    let mut seq_numbers = HashSet::new();  // Use a HashSet to ensure unique entries
    let mut buf = [0; 1500];

    loop {
        match socket.recv_from(&mut buf) {
            Ok((size, _)) => {
                if size < 12 { // Minimum size to contain seq_number.
                    continue;
                }

                let seq_number = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
                if seq_number == u32::MAX { // End-of-transmission signal.
                    break;
                }

                if simulate_loss && loss_packets.contains(&seq_number) {
                    println!("Packet with sequence number {} has been artificially dropped to simulate loss.", seq_number);
                    loss_packets.remove(&seq_number); // Remove from the set to allow future receptions
                    continue; // Do not add to seq_numbers or packets
                }

                let received_checksum = u16::from_be_bytes([buf[10], buf[11]]);
                let packet_data = &buf[12..size];

                let calculated_checksum = calculate_checksum(packet_data);
                if calculated_checksum == received_checksum {
                    packets.push(packet_data.to_vec());
                    seq_numbers.insert(seq_number); // Add valid and non-dropped sequence number
                    println!("Packet {} received with correct checksum: expected {}, got: {}", seq_number, calculated_checksum, received_checksum);
                } else {
                    println!("Checksum mismatch for packet {}: expected {}, got {}", seq_number, calculated_checksum, received_checksum);
                }
            },
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                continue; // Handle timeout
            },
            Err(e) => return Err(e), // Handle other errors
        }
    }
    Ok((packets, seq_numbers)) // Return both the data and their sequence numbers
}






fn write_to_file(path: &str, packets: &HashMap<u32, Vec<u8>>, count: u32) -> io::Result<()> {
    let exe_path = env::current_exe()?;
    let exe_dir = exe_path.parent().ok_or(io::Error::new(io::ErrorKind::Other, "Failed to get executable directory"))?;
    let files_dir = exe_dir.join("../../src/client_files");

    fs::create_dir_all(&files_dir)?;
    let file_path = files_dir.join(path);
    let mut file = File::create(&file_path)?;

    for i in 0..count {
        if let Some(data) = packets.get(&i) {
            file.write_all(data)?;
        }
    }

    Ok(())
}

fn calculate_checksum(data: &[u8]) -> u16 {
    let sum: u32 = data
        .chunks(2)
        .fold(0, |acc, chunk| {
            let word = chunk
                .iter()
                .enumerate()
                .fold(0u16, |word_acc, (i, &byte)| word_acc | ((byte as u16) << ((1 - i) * 8)));
            acc + word as u32
        });

    let wrapped_sum = (sum & 0xFFFF) + (sum >> 16);
    let wrapped_sum = (wrapped_sum & 0xFFFF) + (wrapped_sum >> 16); // Wrap around again if necessary
    !wrapped_sum as u16
}

// Function to calculate the expected number of packets based on data size or last sequence number
fn calculate_expected_number_of_packets(received_seq_numbers: &HashSet<u32>) -> u32 {
    if received_seq_numbers.is_empty() {
        0
    } else {
        // Since HashSet does not support direct indexing or `.max()`, convert to Vec for processing
        let max_seq_num = *received_seq_numbers.iter().max().unwrap();
        max_seq_num + 1
    }
}


fn identify_missing_packets(received_seq_numbers: &HashSet<u32>, expected_num_packets: u32) -> Vec<u32> {
    // Debugging: Print the received sequence numbers and the expected count
    println!("Received sequence numbers: {:?}", received_seq_numbers);
    println!("Expected number of packets: {}", expected_num_packets);

    let missing_packets = (0..expected_num_packets)
        .filter(|n| !received_seq_numbers.contains(n))
        .collect::<Vec<u32>>();

    // Debugging: Print the missing packets found
    println!("Missing packets: {:?}", missing_packets);

    missing_packets
}

// Function to request retransmission for missing packets
fn request_retransmission(socket: &UdpSocket, server_addr: &str, missing_packets: &[u32]) -> io::Result<()> {
    if missing_packets.is_empty() {
        return Ok(());
    }
    let request_string = format!("RETRANSMIT {}", missing_packets.iter().map(|num| num.to_string()).collect::<Vec<_>>().join(","));
    println!("Requesting retransmission for packets: {}", request_string);
    socket.send_to(request_string.as_bytes(), server_addr)?;
    Ok(())
}