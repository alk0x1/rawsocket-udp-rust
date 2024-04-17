use std::collections::HashSet;
use std::io::{self, stdin, Write};
use std::net::UdpSocket;
use std::{env, fs, str};
use std::time::Duration;
use std::fs::File;

fn main() -> io::Result<()> {
	println!("Enter the server IP address and port (e.g., '127.0.0.1:8083'):");
	let server_addr = read_input()?;
	
	println!("Enter the name of the file to retrieve from the server:");
	let filename = read_input()?;
	
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
	
    let (data_received, received_seq_numbers) = receive_response(&socket, simulate_loss, &loss_packets)?;
    println!("received_seq_numbers: {:?}", received_seq_numbers);


	let file_path = format!("{}", filename); // Define o nome do arquivo baseado na entrada do usuário
	    println!("Data received and processed. Number of packets: {}", data_received.len());

	match write_to_file(&filename, data_received) {
		Ok(_) => println!("Arquivo '{}' salvo com sucesso.", file_path),
		Err(err) => println!("Error on save file: {}", err)
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

fn receive_response(socket: &UdpSocket, simulate_loss: bool, loss_packets: &HashSet<u32>) -> io::Result<(Vec<Vec<u8>>, Vec<u32>)>  {
    // 0-3: seq_number
    // 4-7: src_port e dst_port
    // 8-9: length
    // 10-11: checksum
    // 12-end: data
    let mut packets = Vec::new();
    let mut seq_numbers = Vec::new();  // Store sequence numbers of the received packets
    let mut buf = [0; 1500];

    loop {
        match socket.recv_from(&mut buf) {
            Ok((size, _)) => {
                if size < 12 { // Tamanho mínimo para conter o seq_number.
                    continue;
                }

                let seq_number = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
                println!("Seq number: {}", seq_number);
                
                if simulate_loss && loss_packets.contains(&seq_number) {
                    println!("Packet with sequence number {} has been artificially dropped to simulate loss.", seq_number);
                    continue; // Descarta o pacote especificado pelo usuário
                }
                // Verifica se é o pacote de encerramento
                if seq_number == u32::MAX {
                    break;
                }

                let received_checksum = u16::from_be_bytes([buf[10], buf[11]]);
                let packet_data = &buf[12..size]; // Os dados começam após o checksum.

                let calculated_checksum = calculate_checksum(packet_data);
                if calculated_checksum != received_checksum {
                    println!("Checksum mismatch for packet {}: expected {}, got {}", seq_number, calculated_checksum, received_checksum);
                    continue; // Pode escolher descartar este pacote ou solicitar reenvio.
                }

                packets.push(packet_data.to_vec());
                seq_numbers.push(seq_number);
                println!("Packet {} received with correct checksum: expected {}, got: {}", seq_number, calculated_checksum, received_checksum);

            },
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                // Se o socket não receber dados dentro do período de timeout especificado.
                continue; // Você pode decidir adicionar uma lógica de tentativas aqui.
            },
            Err(e) => return Err(e), // Outros erros são propagados.
        }
    }
    Ok((packets, seq_numbers)) // Return both the data and their sequence numbers
}


fn write_to_file(relative_path: &str, packets: Vec<Vec<u8>>) -> io::Result<()> {
    let exe_path = env::current_exe()?;
    let exe_dir = exe_path.parent().ok_or(io::Error::new(io::ErrorKind::Other, "Falha ao obter o diretório do executável"))?;
    let files_dir = exe_dir.join("../../src/client_files");

    fs::create_dir_all(&files_dir)?;
    let file_path = files_dir.join(relative_path);
    let mut file = File::create(file_path)?;

    // Vamos assumir que os pacotes estão na ordem correta.
    // Se não estiverem, você precisará ordená-los antes de escrever.
    for packet in packets {
        file.write_all(&packet)?;
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

