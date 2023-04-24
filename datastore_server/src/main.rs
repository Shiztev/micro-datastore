mod datastore_handle;
mod protocol;

use std::{net::{UdpSocket, SocketAddr}, fs::File};
use datastore_handle::*;
use protocol::{SLEEP_TIME, create_pkt, FLAG_404, FLAG_500, get_seq, ACK};

use crate::protocol::{create_header, FIN};

/// ASCII value for a carriage return
const CR: u8 = 13;

/// Minimum Ethernet MTU in bytes
const ETHER_MTU: usize = 1500;

/// UDP header length in bytes
const UDP_H_LEN: usize = 8;

/// IP header length in bytes
const IP_H_LEN: usize = 20;

/// Minimum MTU in bytes
const MTU: usize = ETHER_MTU - UDP_H_LEN - IP_H_LEN;

/// Length of flags field in bytes
const FLAGS_LEN: usize = 1;

/// Length of sequence number field in bytes
const SEQ_LEN: usize = 8;

/// Length of body field in bytes
const BODY_LEN: usize = MTU - FLAGS_LEN - SEQ_LEN;

/// Starting byte position of body field
const BODY_START: usize = FLAGS_LEN + SEQ_LEN;


/// An enumeration of supported operations between a
/// proxy and the datastore.
#[derive(Debug)]
#[derive(PartialEq)]
enum Op {
  GET(String),
  POST(String),
  FIN,
  ACK,
  NA(u8, u64),
}


/// Enum of the possible errors.
#[derive(Debug)]
#[derive(PartialEq)]
pub enum ReadData {
  MORE,
  DONE,
}


/// Enum of the possible errors.
#[derive(Debug)]
#[derive(PartialEq)]
pub enum Serr {
  DNE(String),
  SERVER(String),
}


/// Handle requests sent to the datastore.
fn main() {
  // receive and handle connections
  loop {
    let socket = match UdpSocket::bind("0.0.0.0:41000") {
      Ok(s) => s,
      Err(_) => {
        eprintln!("Unable to bind a UDP socket to address");
        return;
      }
    };

    handle_error(&socket, receive_connections(&socket));
  }
}


fn get_filename(buf: &[u8; MTU]) -> Result<String, Serr> {
  let i: usize = match buf.iter().position(|&x| x == CR) {
    Some(i) => i,
    None => return Err(Serr::SERVER(format!("Cannot determine filename"))),
  };
  
  Ok(bytes_to_str(buf, BODY_START, i))
}

/// Determines the operation to perform and the file location to perform
/// the operation at.
/// 
/// The expected format of the buffer is:
/// <OP><PATH<CR><LF>
fn determine_op(length: usize, buf: &[u8; MTU]) -> Result<Op, Serr> {
  let flags: u8 = buf[0];

  match flags {
    protocol::GET => Ok(Op::GET(get_filename(buf)?)),
    protocol::POST => Ok(Op::POST(get_filename(buf)?)),
    protocol::FIN => Ok(Op::FIN),
    protocol::ACK => Ok(Op::ACK),
    _ => Ok(Op::NA(flags, get_seq(&buf)?)),
  }
}


fn receive_connections(socket: &UdpSocket) -> Result<(), Serr> {
  let mut buf: [u8; MTU] = [0; MTU];
  let mut length: usize;
  let mut addr: SocketAddr;

  loop {
    // receive datagram
    socket.set_read_timeout(None).expect("System doesn't support set_read_timeout. Please update rust to at least v1.4.0.");
    (length, addr) = match socket.recv_from(&mut buf) {
      Ok(r) => r,
      Err(_) => { continue; },
    };

    // connect and ensure read can timeout
    socket.set_read_timeout(Some(SLEEP_TIME)).expect("System doesn't support set_read_timeout. Please update rust to at least v1.4.0.");
    match socket.connect(addr) {
      Ok(()) => (),
      Err(_) => { eprintln!("Could not connect to {}", addr); continue; },
    }

    return match determine_op(length, &buf)? {
      Op::GET(f) => {
        println!("Received GET request for {}", f);
        let file: File = match File::open(&f) {
          Ok(f) => f,
          Err(_) => return Err(Serr::DNE(format!("{} does not exist", f))),
        };
        let file_size = match file.metadata() {
          Ok(i) => i,
          Err(_) => return Err(Serr::DNE(format!("could not fetch metadata for {}", f))),
        };
        handle_get(f, file, file_size.len(), &socket)
      },

      Op::POST(f) => {
        println!("Received POST request for {}", f);
        handle_post(f, &socket, &buf)
      },

      Op::FIN => {
        println!("Received stale FIN for {0}\nSending FIN for {0} to clean up connection", get_filename(&buf)?);
        socket.send(&buf);
        Ok(())
      },

      Op::ACK => {
        println!("Received stale ACK\nSending FIN to clean up connection");
        socket.send(&create_header(FIN, get_seq(&buf)?));
        Ok(())
      },

      Op::NA(flag, seq) => {
        //Ok(())
        Err(Serr::SERVER(format!("Invalid request initializing flag: {}", flag)))
      },
    };
  }
}


/// Send an error to the proxy if an error occurs.
fn handle_error(socket: &UdpSocket, r: Result<(), Serr>) {
  match r {
    Ok(_) => (),
    Err(e) => send_error(socket, e),
  }
}


/// Send the respective error for the server error over the
/// provided socket.
fn send_error(socket: &UdpSocket, serr: Serr) {
  let err_msg: String = match serr {
    Serr::DNE(e) => { send_404_error(socket); e},
    Serr::SERVER(e) => { send_500_error(socket); e},
  };
  eprintln!("{}", err_msg);
}


/// Send a 404 Error over the provided socket.
fn send_404_error(socket: &UdpSocket) {
  //respond(ERROR_404, socket, "Interrupted while sending 404 response");
  let buf: [u8; MTU] = create_pkt(FLAG_404, 0, &[0; BODY_LEN]);
  socket.send(&buf);
}


/// Send a 500 Error over the provided socket.
fn send_500_error(socket: &UdpSocket) {
  //respond(ERROR_500, socket, "Interrupted while sending 500 response");
  let buf: [u8; MTU] = create_pkt(FLAG_500, 0, &[0; BODY_LEN]);
  socket.send(&buf);
}
