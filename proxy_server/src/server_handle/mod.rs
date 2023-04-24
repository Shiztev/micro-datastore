use std::{net::{TcpStream, UdpSocket}, fs::{File, remove_file}, io::Read};

use crate::{Serr, respond, protocol::{create_pkt, FLAGS_LEN, SEQ_LEN, GET, SYNACK, BODY_LEN, receive::receive, SLEEP_TIME, send_buf, FLAG_404, POST, ACK, send::send, FLAG_500, filename_as_body}, MTU, CRLF};

/// ASCII values for Location: 
const LOC: [u8; 10] = [76, 111, 99, 97, 116, 105, 111, 110, 58, 32];

/// Success 200 response
const OK_200: &[u8] = "HTTP/1.1 200 OK\r\n".as_bytes();

/// Success 201 response
const CREATED_201: &[u8] = "HTTP/1.1 200 CREATED\r\n".as_bytes();

/// Two sets of Carriage-Returns and Line Feeds
const DOUBLE_CRLF: &[u8] = "\r\n\r\n".as_bytes();


/// Print the contents of a byte buffer.
/// Debugging tool.
fn _print_buf(buf: &[u8]) {
  for i in buf {
    println!("{} {}", *i as char, i);
  }
}


/// Sends a datagram over the UDP socket that is connected
/// to the datastore server.
/// 
/// Sends an error back to the client based on the operation being performed.
fn send_datagram(buf: &[u8], socket: &UdpSocket) -> Result<usize, Serr> {
  match socket.send(buf) {
    Ok(i) => Result::Ok(i),
    Err(_) => Result::Err(Serr::SERVER("UDP socket is not connected, cannot read from UDP socket".to_string()))
  }
}


/// Read a datagram from the UDP socket, return the number
/// of bytes and the content buffer.
/// 
/// Sends an error back to the client based on the operation being performed.
fn read_datagram(socket: &UdpSocket) -> Result<(usize, [u8; MTU]), Serr> {
  let mut buf: [u8; MTU] = [0; MTU];

  match socket.recv(&mut buf) {
    Ok(amt) => Result::Ok((amt, buf)),
    Err(_) => {
      Result::Err(Serr::SERVER("UDP socket is not connected, cannot read from UDP socket".to_string()))
    }
  }
}


/// Get sequence number as a u64.
fn get_seq(buf: &[u8; MTU]) -> Result<u64, Serr> {
  let bytes = buf[FLAGS_LEN..FLAGS_LEN + SEQ_LEN]
  .try_into();

  return match bytes {
    Ok(i) => Ok(u64::from_be_bytes(i)),
    Err(_) => Err(Serr::SERVER(format!("out of bounds: there were not 8 bytes between starting index {} and end of buffer of size {}", FLAGS_LEN, buf.len())))
  };
}


/// Responds to an HTTP GET request.
/// 
/// Message format: {"GET", "/path/parts", "more/if/spaces", ..., "HTTP/1.1"}
pub fn handle_get(filename: String, stream: &TcpStream, socket: &UdpSocket) -> Result<(), Serr> {
  let mut buf: [u8; MTU];
  let size: u64;

  // request = [&GET.to_be_bytes(), 0u64.to_be_bytes(), filename.as_bytes(), &crate::CRLF]
  let data: [u8; BODY_LEN] = filename_as_body(&filename)?;
  buf = create_pkt(GET, 0, &data);

  // send request until Flags = 160 (syn & ack)
  buf = send_buf(socket, &buf, SYNACK, &filename)?;

  // get length from this ack (seq #)
  size = get_seq(&buf)?;

  // receive data
  receive(socket, filename.clone(), size)?;

  // <OK_200>Content-Length: <size>\r\n\r\n<buf>
  let response: &Vec<u8> = &[OK_200, &crate::CLEN, size.to_string().as_bytes(), DOUBLE_CRLF].concat();
  respond(&response, &stream, "Interrupted while responding to a GET request");
  send_file_to_client(&filename, stream)?;
  remove_file(&filename);

  println!("Successfully responded to {} GET", filename);
  Result::Ok(())
}


/// Responds to an HTTP POST request.
pub fn handle_post(filename: String, length: u64, stream: &TcpStream, socket: &UdpSocket) -> Result<(), Serr> {
  let file: File = match File::open(&filename) {
    Ok(f) => f,
    Err(e) => return Err(Serr::SERVER(format!("could not open {}:\n{}", filename, e))),
  };

  // request = syn post seq#=len body=filename
  let data: [u8; BODY_LEN] = filename_as_body(&filename)?;
  let buf: [u8; MTU] = create_pkt(POST, length, &data);
  // send request until Flags = 128 (ack)
  send_buf(socket, &buf, ACK, &filename)?;

  // call send
  send(socket, file, filename.clone(), length)?;

  // <CREATED_201>Location: <filename>\r\nContent-Length: <size>\r\n\r\n<buf>
  let response: &Vec<u8> = &[CREATED_201, &LOC, filename.as_bytes(), &crate::CRLF, &crate::CLEN, length.to_string().as_bytes(), DOUBLE_CRLF].concat();
  respond(&response, &stream, "Interrupted while responding to a POST request");
  send_file_to_client(&filename, stream)?;
  remove_file(&filename);

  println!("Successfully responded to {} POST", filename);
  Result::Ok(())
}


fn send_file_to_client(filename: &String, stream: &TcpStream) -> Result<(), Serr> {
  let mut amt: usize;
  let mut file: File = match File::open(&filename) {
    Ok(f) => f,
    Err(_) => return Err(Serr::SERVER(format!("Could not open {} which was already fetched from datastore", &filename)))
  };
  let mut buf: Vec<u8>;

  loop {
    buf = vec![0u8; MTU];
    amt = match file.read(&mut buf) {
      Ok(i) => i,
      Err(_) => return Err(Serr::SERVER(format!("Could not read from {} which was already fetched from datastore", &filename))),
    };

    if amt == 0 {
      break;
    }
    respond(&buf , stream, "Unable to sent data to client");
  }
  Ok(())
}