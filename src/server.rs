use std::fs::File;
use std::io::{self, Read};
use std::net::{UdpSocket, SocketAddr};

struct UdpPacket {
  src_port: u16, // Porta de origem, 16 bits
  dst_port: u16, // Porta de destino, 16 bits
  length: u16,   // Comprimento do cabeçalho UDP + dados, 16 bits
  checksum: u16, // Checksum, 16 bits (opcional, pode ser zero se não usado)
  data: Vec<u8>, // Dados do pacote, representado como um vetor de bytes
}

impl UdpPacket {
   fn new(src_port: u16, dst_port: u16, data: Vec<u8>) -> UdpPacket {
        let length = (8 + data.len()) as u16; // O cabeçalho UDP tem 8 bytes
        UdpPacket {
            src_port,
            dst_port,
            length,
            checksum: 0, // Inicialmente definido como 0, pode ser calculado depois
            data,
        }
    }

}


fn main() -> io::Result<()> {
  // Bind
  let socket = UdpSocket::bind("127.0.0.1:8083")?;

  println!("listening in 127.0.0.1:8083...");

  // Listen
  loop {
    let mut buf = [0u8; 2048];
    let (size, client_address) = socket.recv_from(&mut buf)?;
    let request = std::str::from_utf8(&buf[..size]).unwrap_or_default();
    println!("request: {}", request);

    // Parse Request
    if request.starts_with("GET /") {
      let filename = &request[5..].trim();

      match send_file(&socket, filename, &client_address) {
        Ok(_) => println!("File sent"),
        Err(e) => eprintln!("Error sending file '{}': {}", filename, e),
      }
    }
  }
}

fn send_file(socket: &UdpSocket, filename: &str, client_address: &SocketAddr) -> io::Result<()> {
  let path = format!("/home/aces/Desktop/projects/rawsocket-udp-rust/src/files/{}", filename);

  match File::open(&path) {
    Ok(mut file) => {
      let mut chunk_number = 0;
      let mut buffer = [0u8; 1400]; // Adjusting for MTU - headers

      while let Ok(bytes_read) = file.read(&mut buffer) {
        if bytes_read == 0 {
          break;
        }

        // Simulating a simple checksum as an example; in a real case, you should calculate based on the data
        let checksum = bytes_read as u32 % 256;
        let chunk_header = format!("{:04}:{:04}:{:04}:", chunk_number, bytes_read, checksum);
        let mut packet = chunk_header.as_bytes().to_vec();
        packet.extend_from_slice(&buffer[..bytes_read]);
        println!("checksum: {}", checksum);
        println!("chunk_header: {}", chunk_header);
        println!("packet: {:?}", packet);

        socket.send_to(&packet, client_address)?;
        chunk_number += 1;
      }

      // Indicating end of file
      socket.send_to(b"EOF", client_address)?;
    }
    Err(_) => {
      // Informing the client that the file was not found
      socket.send_to(b"ERR: File Not Found", client_address)?;
    }
  }
  Ok(())
}
