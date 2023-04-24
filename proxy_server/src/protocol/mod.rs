pub mod send;
pub mod receive;

use core::time;
use std::{time::Duration, net::UdpSocket};

use crate::{MTU, Serr};

/// ASCII value for line feed
pub const LF: u8 = 10;

/// ASCII value for carriage return
const CR: u8 = 13;

/// Byte sequence of <CR><LF>
pub const CRLF: [u8; 2] = [CR, LF];

/// Length of flags field in bytes
pub const FLAGS_LEN: usize = 1;

/// Length of sequence number field in bytes
pub const SEQ_LEN: usize = 8;

pub const HEADER_LEN: usize = FLAGS_LEN + SEQ_LEN;

/// Length of body field in bytes
pub const BODY_LEN: usize = MTU - HEADER_LEN;

/// Body length as a u64
const BODY_LEN_U64: u64 = BODY_LEN as u64;

/// Starting byte position of body field
const BODY_START: usize = HEADER_LEN;

/// Size of the window buffer
const WINDOW_SIZE: usize = 5;

/// Flags field value for SYN
const SYN: u8 = 32;

/// Flags field value for GET
pub const GET: u8 = SYN | 8;

/// Flags field value for POST
pub const POST: u8 = SYN | 16;

/// Flags field value for ACK
pub const ACK: u8 = 128;

/// Flags field value for SYN ACK
pub const SYNACK: u8 = ACK | SYN;

/// Flag indicating done with connection
const DONE: u8 = 1;

/// Flag for file not existing
pub const FLAG_404: u8 = 4 | DONE;

/// Flag for server error
pub const FLAG_500: u8 = 2 | DONE;

/// Flag for terminating connection
pub const FIN: u8 = ACK | DONE;

/// Flags field value for DATA
const DATA: u8 = 64;

/// Amount of time to wait in milliseconds
const WAIT_TIME: u64 = 250;

/// Duration of time to sleep
pub const SLEEP_TIME: Duration = time::Duration::from_millis(WAIT_TIME);


pub fn filename_as_body(filename: &String) -> Result<[u8; BODY_LEN], Serr> {
  let mut data: [u8; BODY_LEN] = [0; BODY_LEN];
  let file_bytes: &[u8] = filename.as_bytes();
  let length: usize = file_bytes.len();

  if length > (BODY_LEN - 2) {  // account for trailing <CR><LF>
    return Err(Serr::SERVER(format!("filename exceeds {} bytes, cannot fit into packet", (BODY_LEN - 2))));
  }

  for i in 0..file_bytes.len() {
    data[i] = file_bytes[i];
  }

  data[length] = CRLF[0];
  data[length + 1] = CRLF[1];

  Ok(data)
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


pub fn create_pkt(flag: u8, seq: u64, data: &[u8; BODY_LEN]) -> [u8; MTU] {
  let mut pkt: [u8; MTU] = [0; MTU];
  let mut index = FLAGS_LEN;
  let seq_bytes: [u8; SEQ_LEN] = seq.to_be_bytes();

  pkt[0] = flag;

  for i in 0..SEQ_LEN {
    pkt[index + i] = seq_bytes[i];
  }

  index += SEQ_LEN;

  for i in 0..BODY_LEN {
    pkt[index + i] = data[i];
  }

  pkt
}


fn create_ack(seq: u64) -> [u8; MTU] {
  let mut pkt: [u8; MTU] = [0; MTU];
  let seq_bytes: [u8; SEQ_LEN] = seq.to_be_bytes();

  pkt[0] = ACK;

  for i in 0..SEQ_LEN {
    pkt[FLAGS_LEN + i] = seq_bytes[i];
  }

  pkt
}


/// Calculates the index into the window w/r/t the current
/// starting sequence number and the received sequence number.
fn calculate_index(seq: u64, start: u64) -> usize {
  (seq - start) as usize / BODY_LEN
}


/// Parse out the data portion of a packet.
fn get_body(pkt: &[u8; MTU]) -> [u8; BODY_LEN] {
  let tmp: &[u8] = &pkt[BODY_START..];  // MTU - BODY_START = BODY_LEN
  tmp.try_into().expect("Unable to get body array from packet")
}


/// Send a buffer over the provided socket, ensuring its delivery.
pub fn send_buf(socket: &UdpSocket, buf: &[u8; MTU], flags: u8, filename: &String) -> Result<([u8; MTU]), Serr> {
  let mut amt;
  let mut received: [u8; MTU];

  loop {
    received = [0; MTU];
    match socket.send(buf) {
      Ok(_) => (),
      Err(_) => (),
    }

    // read ack
    amt = match socket.recv(&mut received) {
      Ok(i) => i,
      Err(_) => continue,
    };

    if amt >= FLAGS_LEN + SEQ_LEN {  // don't require ack to have body
      // if flags match
      if received[0] == flags {
        return Ok(received)
      }

      if received[0] == FLAG_404 {
        return Err(Serr::DNE(format!("{} does not exist", filename)));
      }

      if received[0] == FLAG_500 {
        return Err(Serr::SERVER(format!("error with {}", filename)));
      }
    }
  }
}
