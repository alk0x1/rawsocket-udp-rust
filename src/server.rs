use std::io::{self, Read};
use std::fs::File;
use std::mem::MaybeUninit;
use socket2::{Socket, Domain, Type, SockAddr};
use std::net::SocketAddr;

fn main() -> io::Result<()> {
  let socket = Socket::new(Domain::IPV4, Type::DGRAM, None)?;
  let address: SocketAddr = "127.0.0.1:8083".parse().map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
  socket.bind(&SockAddr::from(address))?;

  let mut buf: [MaybeUninit<u8>; 1024] = unsafe {
    MaybeUninit::uninit().assume_init()
  };

  loop {
    println!("Listening on 8083...");
    let (size, client_address) = socket.recv_from(&mut buf)?;
    let request = unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, size) };
    if let Err(e) = handle_request(&socket, request, &client_address) {
      eprintln!("Error handling request: {}", e);
    }

    println!("Received {} bytes from {:?}", size, client_address);
  }
}

fn handle_request(socket: &Socket, request: &[u8], client_address: &SockAddr) -> io::Result<()> {
  let request_str = String::from_utf8_lossy(request);

  if request_str.starts_with("GET /") {
    let filename = request_str[5..].trim(); // Extract filename from the request
    println!("filename: {}", filename);
    send_file(socket, filename, client_address)?;
  }
  Ok(())
}

fn send_file(socket: &Socket, filename: &str, client_address: &SockAddr) -> io::Result<()> {
  let path = String::from("/home/aces/Desktop/projects/rawsocket-udp-rust/src/files/") + filename;
  println!("path: {}", path);
  let mut file = File::open(path)?;
  let mut file_buf = Vec::new();
  file.read_to_end(&mut file_buf)?;

  // This is a simplified example; you should chunk the file for large files.
  socket.send_to(&file_buf, client_address)?;
  Ok(())
}
