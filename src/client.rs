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
	
	let message = format!("GET /{}", filename);
	let socket = UdpSocket::bind("0.0.0.0:0")?;
	socket.set_read_timeout(Some(Duration::from_secs(5)))?;
	
	send_request(&socket, &server_addr, &message)?;
	
	let data_received = receive_response(&socket)?;

	// calculate_checksum(&data_received);

	let file_path = format!("{}", filename); // Define o nome do arquivo baseado na entrada do usuário
	
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

fn receive_response(socket: &UdpSocket) -> io::Result<Vec<Vec<u8>>>  {
    let mut packets = Vec::new();
    loop {
        let mut buf = [0; 1500];
        match socket.recv_from(&mut buf) {
            Ok((size, _)) => {
                if size < 4 { // Tamanho mínimo para conter o seq_number.
                    continue; // Se o pacote for muito pequeno para ter um seq_number, ignora.
                }

                let seq_number = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
                println!("Seq number: {}", seq_number);

                // Verifica se é o pacote de encerramento
                if seq_number == u32::MAX {
									println!("entrou no if");
									break; // Final da transmissão, sai do loop.
								}

                let packet = &buf[4..size]; // Exclui os 4 bytes do número de sequência
                packets.push(packet.to_vec());
            },
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                // Se o socket não receber dados dentro do período de timeout especificado.
                continue; // Você pode decidir adicionar uma lógica de tentativas aqui.
            },
            Err(e) => return Err(e), // Outros erros são propagados.
        }
    }
    Ok(packets)
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