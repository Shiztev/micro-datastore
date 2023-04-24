mod buffer;

use std::net::UdpSocket;

use crate::{MTU, Serr, protocol::WINDOW_SIZE, ReadData};

use self::buffer::Buf;

use super::{create_ack, FLAG_500, create_pkt, filename_as_body, FIN, HEADER_LEN, FLAG_404};


/// Receive data via UDP socket.
/// If all data read successfully, returns Ok(())
pub fn receive(socket: &UdpSocket, filename: String, size: u64) -> Result<(), Serr> {
  let mut amt: usize;
  let mut buf: [u8; MTU] = [0; MTU];
  let mut data_buf: Buf = Buf::new(filename.clone(), size)?;
  let mut ack_seq: u64;

  loop {
    // save sequential data read so far, send ACK, wait for data to be sent
    ack_seq = data_buf.save_read_data(socket)?;
    ack(socket, ack_seq);

    // add new data to window while data is read in
    for _ in 0..WINDOW_SIZE {
      amt = match socket.recv(&mut buf) { // read in data
        Ok(i) => i,
        Err(_) => {  // packet loss
          continue;
        },
      };
      if !(amt > 0) { break; }

      if buf[0] == FLAG_500 {
        return Err(Serr::SERVER("Received server error".to_string()));
      }

      match data_buf.add(&buf)? {  // add data to window
        ReadData::MORE => (),
        ReadData::DONE => {  // add data received, done
          ack_seq = data_buf.save_read_data(socket)?;
          return done(socket, ack_seq, &filename);
        },
      };
    }
  }
}


fn done(socket: &UdpSocket, seq: u64, filename: &String) -> Result<(), Serr> {
  let final_ack: [u8; MTU] = create_pkt(FIN, seq, &filename_as_body(filename)?);
  let mut buf: [u8; MTU];
  let mut amt: usize;

  loop {
    socket.send(&final_ack);

    buf = [0; MTU];
    amt = match socket.recv(&mut buf) {
      Ok(i) => i,
      Err(_) => continue,
    };

    if amt == 0 { continue; }

    if buf[0] == FLAG_500 { return Err(Serr::SERVER(format!("Received error 500 while terminating"))); }
    else if buf[0] == FLAG_404 { return Err(Serr::SERVER(format!("Received error 404 while terminating"))); }
    else if buf == final_ack {
      println!("Received file from datastore");
      return Ok(());
    }
  }
}


/// ACK's the next smallest expected byte.
pub fn ack(socket: &UdpSocket, seq: u64) -> Result<u64, Serr> {
  let buf: [u8; MTU] = create_ack(seq);
  match socket.send(&buf){
    Ok(_) => Ok(0),
    Err(_) => Err(Serr::SERVER("UDP socket is not connected, cannot read from UDP socket".to_string())),
  }
}