use std::{fs::{File}, net::UdpSocket, io::Write};

use crate::{Serr, protocol::{WINDOW_SIZE, get_seq, DATA, BODY_LEN, BODY_LEN_U64, calculate_index, get_body}, MTU, ReadData};

use super::ack;


pub struct Buf {
  pub filename: String,
  file: File,
  size: u64,  // number of bytes of file
  received: u64,  // number of bytes received
  start: u64,  // next expected byte
  indicies: Vec<(bool, u64)>,  // sequence numbers/byte positions, if false -> not yet received
  data: Vec<[u8; BODY_LEN]>,  // data

}


impl Buf {
  /// Create new auto-saving buffer.
  pub fn new(f: String, s: u64) -> Result<Buf, Serr> {
    let opened_file: File = match File::create(&f) {
      Ok(f) => f,
      Err(_) => return Result::Err(Serr::SERVER(format!("Unable to open {}", f))),
    };

    Ok(Buf { filename: f, file: opened_file, size: s, received: 0, start: 0, indicies: vec![(false, 0); WINDOW_SIZE], data: vec![[0; BODY_LEN]; WINDOW_SIZE], })
  }


  /// Save data that's in sequential order to disk, and ACK next expected byte.
  /// Returns number of bytes written to disk.
  pub fn save_read_data(&mut self, socket: &UdpSocket) -> Result<u64, Serr> {
    let mut index: (bool, u64);
    let mut amt: usize;

    for i in 0..WINDOW_SIZE {
      index = self.indicies[0];
      if !index.0 { break; }

      // expect to save BODY_LEN amount of data
      // unless next byte beyond file size
      amt = self.get_data_size();
      let buf: [u8; BODY_LEN] = self.data.remove(0);

      // save data and update num_bytes_saved
      match self.file.write_all(&buf[..amt]) {
        Ok(_) => (),
        Err(_) => return Result::Err(Serr::SERVER(format!("Unable to write seq {} to {}", index.1, self.filename))),
      };
      drop(buf);

      // shift windows
      self.data.resize(WINDOW_SIZE, [0; BODY_LEN]);
      self.indicies.remove(0);
      self.indicies.resize(WINDOW_SIZE, (false, 0));

      // update counts
      self.start += BODY_LEN_U64;
    }

    Ok(self.start)
  }


  pub fn add(&mut self, buf: &[u8; MTU]) -> Result<ReadData, Serr> {
    let seq: u64;
    let index: usize;

    // ensure data flag set
    if buf[0] != DATA { eprintln!("expected DATA flag ({}) got {}", DATA, buf[0]); return Ok(ReadData::MORE); }
    seq = get_seq(buf)?;

    // drop delayed paket
    if seq < self.start { return Ok(ReadData::MORE); }

    index = calculate_index(seq, self.start);
    // outside window/data already at index -> drop packet
    if index >= self.indicies.len() || self.indicies[index].0 { return Ok(ReadData::MORE); }

    // add to data window
    self.indicies[index] = (true, seq);
    self.data[index] = get_body(buf);
    self.received += BODY_LEN_U64;

    // check if this is last packet
    if self.received >= self.size { return Ok(ReadData::DONE); }

    Ok(ReadData::MORE)
  }


  /// Get expected size of data.
  fn get_data_size(&self) -> usize {
    // self.start + BODY_LEN > self.size => adjust amount
    if (self.start + BODY_LEN_U64) >= self.size {
      return (self.size - self.start).try_into().expect("Snagging byte size of final packet, which is larger than what this system can support. This is a hardware issue.");
    }
    BODY_LEN
  }


  /// ACK's the next smallest expected byte.
  fn ack(&self, socket: &UdpSocket) -> Result<u64, Serr> {
    ack(socket, self.start)
  }


  pub fn is_empty(&self) -> bool {
    !(self.received > 0)
  }


  pub fn _print_seq(&self) {
    for i in 0..WINDOW_SIZE {
      println!("{:?}", self.indicies[i]);
    }
  }
}