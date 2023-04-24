use std::{fs::File, io::Read, net::UdpSocket};

use crate::{protocol::{BODY_LEN, WINDOW_SIZE, DATA, create_pkt, ACK, get_seq, BODY_LEN_U64}, Serr, MTU};


pub struct Buf {
  file: File,
  pub filename: String,
  size: u64,  // size of file
  start: u64,  // next byte to be acked
  indicies: Vec<(bool, u64)>,  // send?, sequence numbers/byte positions
  data: Vec<[u8; BODY_LEN]>,  // data
}


impl Buf {
  /// Create new auto-saving buffer.
  /// 
  /// Fills windows with initial data
  pub fn new(f: File, filename: &String, file_size: u64) -> Result<Buf, Serr> {
    let mut b: Buf = Buf { file: f, filename: filename.clone(), size: file_size, start: 0, indicies: vec![(false, 0); WINDOW_SIZE], data: vec![[0; BODY_LEN]; WINDOW_SIZE], };
    b.fill_window()?;
    Ok(b)
  }


  /// Fill window with data from file.
  fn fill_window(&mut self) -> Result<(), Serr> {
    let mut index: u64 = self.start;
    let mut data_buf: [u8; BODY_LEN];
    let mut amt: usize;


    for i in 0..WINDOW_SIZE {
      if self.indicies[i].0 {
        index = self.indicies[i].1 + BODY_LEN_U64;
        continue;
      }

      // fetch data
      data_buf = [0; BODY_LEN];
      amt = match self.file.read(&mut data_buf) {
        Ok(s) => s,
        Err(_) => return Err(Serr::SERVER(format!("Could not read from {}", self.filename))),
      };

      // check amount read
      if amt == 0 { return Ok(()); }  // read all data from file

      // write to window
      self.indicies[i] = (true, index);
      self.data[i] = data_buf.clone();
      index += BODY_LEN_U64;

      if amt < BODY_LEN { break; }
    }

    Ok(())
  }


  /// Determine if all data sent.
  pub fn is_done(&self) -> bool {
    !self.indicies[0].0 
  }


  /// Send all the data in the window.
  pub fn send(&self, socket: &UdpSocket) {
    let mut pkt: [u8; MTU];
    let mut data: &[u8; BODY_LEN];
    let mut index: (bool, u64);

    // create and send a datagram for each slot in the window
    for i in 0..WINDOW_SIZE {
      index = self.indicies[i];
      if !index.0 { return; }

      data = &self.data[i];
      pkt = create_pkt(DATA, index.1, data);
      let amt = match socket.send(&pkt) {
        Ok(i) => i,
        Err(_) => continue,
      };
    }
  }


  /// Slide the window over with respect to ACK received.
  pub fn adjust(&mut self, buf: &[u8; MTU]) -> Result<(), Serr> {
    let seq: u64;
    let mut index: (bool, u64);

    // ensure ack received
    if buf[0] < ACK { eprintln!("Received non-ACK (ACK types > {}), instead {}", ACK, buf[0]); return Ok(()); }

    // shift windows to index
    seq = get_seq(buf)?;

    for _ in 0..WINDOW_SIZE {
      index = self.indicies[0];
      if !index.0 { return Ok(()); }
      if index.1 >= seq {
        self.start = index.1;
        break;
      }

      self.indicies.remove(0);
      self.data.remove(0);

      if self.indicies.is_empty() {
        self.start += BODY_LEN_U64;
        break;
      }

      if self.indicies[0].0 { self.start = self.indicies[0].1; }  // next thing in window is sendable
      else { break; }  // done, will fill anyways for saftey
    }

    self.indicies.resize(WINDOW_SIZE, (false, 0));
    self.data.resize(WINDOW_SIZE, [0; BODY_LEN]);
    return self.fill_window();  // TODO: if file not empty and self.indicies[0] != seq, ERROR!
  }

  pub fn print_indicies(&self) {
    for i in 0..WINDOW_SIZE {
      println!("{:?}", self.indicies[i]);
    }
  }
}
