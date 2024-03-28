use std::{mem::MaybeUninit, net::SocketAddr, io::Read, fs::{File, metadata}, sync::{Arc, atomic::{AtomicBool, Ordering}}, thread};
use socket2::{Socket, Domain, Type, SockAddr};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, None)?;
    let addr: SocketAddr = "127.0.0.1:8080".parse()?;
    socket.bind(&SockAddr::from(addr))?;

    println!("UDP Server listening on {}", addr);

    loop {
        handle_connection(&socket)?;
    }
}

fn handle_connection(socket: &Socket) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer: [MaybeUninit<u8>; 1024] = unsafe { MaybeUninit::uninit().assume_init() };
    
    let (amt, src) = {
        let buffer_slice = unsafe { &mut *(&mut buffer[..] as *mut [MaybeUninit<u8>] as *mut [u8]) };
        socket.recv_from(buffer_slice).unwrap();
    };

    let received_data = unsafe { 
        std::slice::from_raw_parts(buffer.as_ptr() as *const u8, amt) 
    };
    let received_text = std::str::from_utf8(received_data)?;

    println!("Received: {}", received_text);

    // Example response logic, customize as per your requirements
    if received_text == "Sair" {
        socket.send_to("Connection terminated.".as_bytes(), &src)?;
    } else {
        // Further logic for handling different commands
    }

    Ok(())
}

