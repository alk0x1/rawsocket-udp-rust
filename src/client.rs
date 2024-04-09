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

	let file_path = format!("{}", filename); // Define o nome do arquivo baseado na entrada do usuário
	write_to_file(&filename, &data_received)?;
	
	println!("Arquivo '{}' salvo com sucesso.", file_path);
	
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

fn receive_response(socket: &UdpSocket) -> io::Result<Vec<u8>> {
    let mut data_received = Vec::new();
    loop {
        let mut buf = [0; 1472];
        match socket.recv_from(&mut buf) {
            Ok((size, _)) => {
                if size == 0 { break; } // Condição de saída se nenhum dado for recebido
                data_received.extend_from_slice(&buf[..size]);
                if size < buf.len() {
                    break; // Supondo que um pacote menor que o buffer sinalize o final dos dados
                }
            },
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                // Se o socket não receber dados dentro do período de timeout especificado
                break;
            },
            Err(e) => return Err(e), // Propaga outros erros
        }
    }
    Ok(data_received)
}

fn write_to_file(relative_path: &str, data: &[u8]) -> io::Result<()> {
	let exe_path = env::current_exe()?;
	let exe_dir = exe_path.parent().ok_or(io::Error::new(io::ErrorKind::Other, "Falha ao obter o diretório do executável"))?;
	let files_dir = exe_dir.join("../../src/client_files");
	
	fs::create_dir_all(&files_dir)?;
	
	let file_path = files_dir.join(relative_path);

	let mut file = File::create(file_path)?;
	file.write_all(data)?;
	
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