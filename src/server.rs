use std::fs::File;
use std::io::{self, Read};
use std::mem::MaybeUninit;
use socket2::{Domain, Socket, SockAddr, Type};
use std::net::SocketAddr;

fn main() -> io::Result<()> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, None)?;
    let address: SocketAddr = "127.0.0.1:8083".parse().map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    socket.bind(&SockAddr::from(address))?;

    let mut buf: [MaybeUninit<u8>; 1024] = unsafe { MaybeUninit::uninit().assume_init() };

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
        println!("Handling request for file: {}", filename);
        send_file(socket, filename, client_address)?;
    }
    Ok(())
}

fn send_file(socket: &Socket, filename: &str, client_address: &SockAddr) -> io::Result<()> {
    let path = format!("/home/aces/Desktop/projects/rawsocket-udp-rust/src/files/{}", filename);
    match File::open(&path) {
        Ok(mut file) => {
            let mut chunk_number = 0u32;
            let mut buffer = [0u8; 1024]; // Slightly reduced to fit chunk number and size in the packet
            while let Ok(bytes_read) = file.read(&mut buffer) {
                if bytes_read == 0 {
                    break; // End of file
                }

                // Prepare chunk prefix with chunk number and size (simple for demo purposes)
                let chunk_prefix = format!("{:04}:{:04}:", chunk_number, bytes_read);
                let mut packet = chunk_prefix.into_bytes();
                packet.extend_from_slice(&buffer[..bytes_read]);

                // Send the chunk
                socket.send_to(&packet, client_address)?;

                chunk_number += 1;
            }

            // Send a completion message to signal EOF
            socket.send_to(b"EOF", client_address)?;
        }
        Err(_) => {
            // Send a "File Not Found" message if unable to open the file
            let err_msg = "ERR: File Not Found";
            socket.send_to(err_msg.as_bytes(), client_address)?;
        }
    }
    Ok(())
}
