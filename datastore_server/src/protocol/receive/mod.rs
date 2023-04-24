mod buffer;

use std::net::UdpSocket;

use crate::{MTU, Serr, ReadData, get_filename};

use self::buffer::Buf;

use super::{create_ack, WINDOW_SIZE, DATA, create_pkt, filename_as_body, FIN};


/// Receive data via UDP socket.
/// If all data read successfully, returns Ok(())
pub fn receive(socket: &UdpSocket, filename: String, size: u64) -> Result<(), Serr> {
  let mut amt: usize;
  let mut buf: [u8; MTU] = [0; MTU];
  let mut data_buf: Buf = Buf::new(filename, size)?;
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

      if buf[0] < DATA {
        if data_buf.is_empty() { continue; }  // server hasn't received ack 0
        return Err(Serr::SERVER(format!("Got flag {} while receiving data, canceling POST request", buf[0])));
      }

      match data_buf.add(&buf)? {  // add data to window
        ReadData::MORE => (),
        ReadData::DONE => {  // add data received, done
          ack_seq = data_buf.save_read_data(socket)?;
          return done(socket, ack_seq, &data_buf);
        },
      };
    }
  }
}


/// Terminate connection.
fn done(socket: &UdpSocket, seq: u64, data_buf: &Buf) -> Result<(), Serr> {
  let final_ack = create_pkt(FIN, seq, &filename_as_body(&data_buf.filename)?);
  socket.send(&final_ack);
  Ok(())
}


/// ACK's the next smallest expected byte.
pub fn ack(socket: &UdpSocket, seq: u64) -> Result<u64, Serr> {
  let buf: [u8; MTU] = create_ack(seq);
  match socket.send(&buf){
    Ok(_) => Ok(0),
    Err(_) => Err(Serr::SERVER("UDP socket is not connected, cannot read from UDP socket".to_string())),
  }
}