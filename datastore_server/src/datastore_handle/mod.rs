use std::{net::UdpSocket, fs::{File, remove_file}};
use crate::{MTU, Serr, protocol::{create_pkt, BODY_LEN, SYNACK, send::send, ACK, receive::receive, get_seq, send_buf}, FLAGS_LEN, SEQ_LEN};


/// Handle managing the result of a result.
fn handle_result<T, E>(serr: Serr, r: Result<T, E>, socket: &UdpSocket) -> Result<T, Serr> {
  match r {
    Ok(i) => Result::Ok(i),
    Err(_) => Result::Err(serr),
  }
}


/// Process a GET request.
pub fn handle_get(filename: String, file: File, file_size: u64, socket: &UdpSocket) -> Result<(), Serr> {
  let buf: [u8; MTU];
  let data: [u8; BODY_LEN] = [0; BODY_LEN];

  // send file len (syn & ack) until ack w falgs = 128 (ack)
  buf = create_pkt(SYNACK, file_size, &data);
  send_buf(socket, &buf, ACK)?;

  // call send
  send(socket, file, filename, file_size)
}


/// Process a POST request.
pub fn handle_post(filename: String, socket: &UdpSocket, buf: &[u8; MTU]) -> Result<(), Serr> {
  let size: u64 = get_seq(&buf)?;

  // call receive
  match receive(socket, filename.clone(), size) {
    Ok(_) => {
      println!("Succsefully received {}", filename);
      Ok(())
    },
    Err(e) => {
      remove_file(&filename);
      Err(e)
    }
  }
}


/// Creates a string from a designated slice of a byte buffer.
/// 
/// Inclusive start, exclusive end.
pub fn bytes_to_str(buf: &[u8], start: usize, end: usize) -> String {
  buf[start..end]
    .into_iter()
    .map(|b| *b as char)
    .collect::<String>()
}
