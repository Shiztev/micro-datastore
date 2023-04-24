mod buffer;

use std::{net::UdpSocket, fs::File};

use crate::{MTU, Serr};

use self::buffer::Buf;

use super::{ACK, FIN};


/// Send the provided file via UDP.
pub fn send(socket: &UdpSocket, file: File, filename: String, file_size: u64) -> Result<(), Serr> {
  let mut buf: [u8; MTU] = [0; MTU];
  let mut data_buf: Buf = Buf::new(file, &filename, file_size)?;
  let mut amt: usize;

  loop {
    // send window
    data_buf.send(socket);

    // read ack
    amt = match socket.recv(&mut buf) {
      Ok(i) => i,
      Err(_) => {  // packet lost
        continue
      },
    };

    if amt > 0 {  // adjust window
      // ensure packet isn't a request
      if buf[0] < ACK {
        return Ok(());
      }

      data_buf.adjust(&buf)?;
      if buf[0] == FIN { return terminate(socket, &data_buf, &buf); }
    }
  }
}


/// Terminate connection.
fn terminate(socket: &UdpSocket, data_buf: &Buf, buf: &[u8; MTU]) -> Result<(), Serr> {
  socket.send(buf);
  if data_buf.is_done() { println!("Successfully sent {}", data_buf.filename); }
  else { eprintln!("Received FIN before all data was sent"); }
  Ok(())
}
