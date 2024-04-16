use std::env;
use std::fs::File;
use std::io::{self, Read};
use std::net::{SocketAddr, UdpSocket};
use std::collections::HashMap;
use std::sync::Mutex;

// Constante para indicar o número de sequência de fim de transmissão.
const END_OF_TRANSMISSION_SEQ_NUM: u32 = u32::MAX;

// Macro para uso de variáveis estáticas.
#[macro_use]
extern crate lazy_static;

// Estrutura que representa um pacote UDP.
#[derive(Clone)]
struct UdpPacket {
  seq_number: u32,
  src_port: u16,
  dst_port: u16,
  length: u16,
  checksum: u16,
  data: Vec<u8>,
}

// Armazenamento estático para pacotes, usando um Mutex para acesso seguro entre threads.
lazy_static! {
  static ref PACKETS_STORAGE: Mutex<HashMap<u32, UdpPacket>> = Mutex::new(HashMap::new());
}

// Implementação de métodos para a estrutura UdpPacket.
impl UdpPacket {
  // Construtor para UdpPacket.
  fn new(seq_number: u32, src_port: u16, dst_port: u16, data: Vec<u8>, length: u16, checksum: u16) -> UdpPacket {
    UdpPacket {
      seq_number,
      src_port,
      dst_port,
      length,
      checksum,
      data,
    }
  }

  // Método para serializar um pacote UDP em bytes.
  fn serialize(&self) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&self.seq_number.to_be_bytes());
    bytes.extend_from_slice(&self.src_port.to_be_bytes());
    bytes.extend_from_slice(&self.dst_port.to_be_bytes());
    bytes.extend_from_slice(&self.length.to_be_bytes());
    bytes.extend_from_slice(&self.checksum.to_be_bytes());
    bytes.extend(self.data.clone());

    bytes
  }

  // Método para preparar pacotes a partir de dados brutos.
  fn prepare_packets(src_port: u16, dst_port: u16, data: Vec<u8>) -> Vec<UdpPacket> {
    data.chunks(1472).enumerate().map(|(index, chunk)| {
      let seq_number = index as u32;
      let checksum = UdpPacket::calculate_checksum(chunk);
      let length = chunk.len() as u16 + 8;
      UdpPacket::new(seq_number, src_port, dst_port, chunk.to_vec(), length, checksum)
    }).collect()
  }

  // Método para calcular o checksum de um bloco de dados.
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
    let wrapped_sum = (wrapped_sum & 0xFFFF) + (wrapped_sum >> 16);
    !wrapped_sum as u16
  }
}

// Função principal que configura e executa o servidor UDP.
fn main() -> io::Result<()> {
  let socket = UdpSocket::bind("0.0.0.0:8083")?;
  println!("Escutando em 127.0.0.1:8083...");

  loop {
    let mut buf = [0u8; 2048];
    let (size, client_address) = socket.recv_from(&mut buf)?;
    let request = std::str::from_utf8(&buf[..size]).unwrap_or_default();
    println!("Requisição: {}", request);
    
    if request.starts_with("GET /") {
      let filename = &request[5..].trim();
      match get_file_data(filename) {
          Ok(data) => {
              let packets = UdpPacket::prepare_packets(8083, client_address.port(), data);
              for packet in packets {
                  send_packet(&socket, packet, client_address).expect("Error sending packet...");
              }
              send_end_of_transmission_packet(&socket, client_address)?;
          },
          Err(e) if e.kind() == io::ErrorKind::NotFound => {
              println!("File not found: {}", filename);
              send_error_message(&socket, "Arquivo não encontrado", client_address)?;
          },
          Err(e) => {
              println!("Error reading file: {}", e);
              send_error_message(&socket, &format!("Erro Ao ler o arquivo: {}", e), client_address)?;
          }
        }
    } else if request.starts_with("RETRANSMIT ") {
      println!("Tratando solicitação de retransmissão.");
      handle_retransmission_request(&socket, request, client_address)?;
    }
  }
}

// Funções auxiliares para enviar pacotes, tratar requisições de retransmissão e acessar dados do arquivo.
fn get_packet_for_sequence(seq_number: u32) -> Option<UdpPacket> {
  let packets = PACKETS_STORAGE.lock().unwrap();
  packets.get(&seq_number).cloned()
}

fn send_packet(socket: &UdpSocket, packet: UdpPacket, destination: SocketAddr) -> io::Result<()> {
  let packet_bytes = packet.serialize();
  socket.send_to(&packet_bytes, destination)?;

  let mut packets = PACKETS_STORAGE.lock().unwrap();
  packets.insert(packet.seq_number, packet);

  Ok(())
}

fn send_end_of_transmission_packet(socket: &UdpSocket, destination: SocketAddr) -> io::Result<()> {
  let end_packet = UdpPacket::new(
    END_OF_TRANSMISSION_SEQ_NUM,
    8083,
    destination.port(),
    Vec::new(),
    8,
    0,
  );

  let packet_bytes = end_packet.serialize();
  socket.send_to(&packet_bytes, destination)?;

  Ok(())
}

fn get_file_data(filename: &str) -> io::Result<Vec<u8>> {
    let exe_path = env::current_exe()?;
    let exe_dir = exe_path.parent().ok_or(io::Error::new(io::ErrorKind::Other, "Failed to get executable directory"))?;
    let files_dir = exe_dir.join("../../src/files");
    let path = files_dir.join(filename);
    let mut file = match File::open(&path) {
        Ok(file) => file,
        Err(_) => return Err(io::Error::new(io::ErrorKind::NotFound, "File not found"))
    };

    let mut data = Vec::new();
    file.read_to_end(&mut data)?;
    Ok(data)
}

fn handle_retransmission_request(socket: &UdpSocket, request: &str, client_address: SocketAddr) -> io::Result<()> {
  let sequences: Vec<u32> = request.trim_start_matches("RETRANSMIT ")
                                  .split(',')
                                  .filter_map(|s| s.parse::<u32>().ok())
                                  .collect();

  let mut retransmitted_any = false;

  for seq_number in sequences {
      println!("Verificando sequência de pacotes: {}", seq_number);
      if let Some(packet) = get_packet_for_sequence(seq_number) {
          println!("Retransmitindo pacote para número de sequência: {}", seq_number);
          send_packet(socket, packet, client_address)?;
          retransmitted_any = true;
      } else {
          println!("Nenhum pacote encontrado para número de sequência: {}", seq_number);
      }
  }
  
  if retransmitted_any {
    send_end_of_transmission_packet(socket, client_address)?;
  }

  Ok(())
}

fn send_error_message(socket: &UdpSocket, message: &str, destination: SocketAddr) -> io::Result<()> {
  let error_message = format!("ERROR: {}", message);
  socket.send_to(error_message.as_bytes(), destination)?;
  Ok(())
}