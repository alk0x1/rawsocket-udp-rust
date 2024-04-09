use std::env;
use std::fs::File;
use std::io::{self, Read, Error};
use std::net::{SocketAddr, UdpSocket};

struct UdpPacket {
  src_port: u16, // Porta de origem, 16 bits
  dst_port: u16, // Porta de destino, 16 bits
  length: u16,   // Comprimento do cabeçalho UDP + dados, 16 bits
  checksum: u16, // Checksum, 16 bits (opcional, pode ser zero se não usado)
  data: Vec<u8>, // Dados do pacote, representado como um vetor de bytes
}

impl UdpPacket {
  fn new(src_port: u16, dst_port: u16, data: Vec<u8>, length: u16, checksum: u16, ) -> UdpPacket {
    // let length = (8 + data.len()) as u16; // O cabeçalho UDP tem 8 bytes
    UdpPacket {
      src_port,
      dst_port,
      length,
      checksum, // Inicialmente definido como 0, pode ser calculado depois
      data,
    }
  }

  fn serialize(&self) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&self.src_port.to_be_bytes());
    bytes.extend_from_slice(&self.dst_port.to_be_bytes());
    bytes.extend_from_slice(&self.length.to_be_bytes());
    bytes.extend_from_slice(&self.checksum.to_be_bytes());
    bytes.extend(self.data.clone());

    bytes
  }

  fn prepare_packets(src_port: u16, dst_port: u16, data: Vec<u8>) -> Vec<UdpPacket> {
    data.chunks(1472).map(|chunk| {
      let checksum = UdpPacket::calculate_checksum(chunk);
      // O comprimento é o tamanho dos dados + tamanho do cabeçalho UDP (8 bytes)
      let length = chunk.len() as u16 + 8; // Adicione 8 para incluir o cabeçalho UDP se necessário
      UdpPacket::new(src_port, dst_port, chunk.to_vec(), length, checksum)
    }).collect()
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
    
    if request.starts_with("GET /") {
      let filename = &request[5..].trim();
      let data = get_file_data(filename).expect("Error getting file data");
      let packets = UdpPacket::prepare_packets(8083, client_address.port(), data);

      for packet in packets {
        send_packet(&socket, packet, client_address).expect("Error sending the packet...");
      }

    }
   
  }
}

fn send_packet(socket: &UdpSocket, packet: UdpPacket, destination: SocketAddr) -> io::Result<()> {
  let packet_bytes = packet.serialize();

  socket.send_to(&packet_bytes, destination).unwrap();

  Ok(())
}

fn get_file_data(filename: &str) -> Result<Vec<u8>, Error> {
  // Obtém o caminho executável atual
  let exe_path = env::current_exe()?;
  let exe_dir = exe_path.parent().ok_or(io::Error::new(io::ErrorKind::Other, "Falha ao obter o diretório do executável"))?;
  let files_dir = exe_dir.join("../../src/files");
  println!("Caminho para o diretório 'files': {}", files_dir.display());

  // Constrói o caminho completo para o arquivo
  let path = files_dir.join(filename); // Usa `join` para evitar problemas com espaços

  // Abre o arquivo
  let mut file = File::open(path)?;

  // Lê os dados do arquivo
  let mut data = Vec::new();
  file.read_to_end(&mut data)?;

  Ok(data)
}
