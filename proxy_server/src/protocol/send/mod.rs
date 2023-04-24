mod buffer;

use std::{net::UdpSocket, fs::File};

use crate::{MTU, Serr, ReadData};

use self::buffer::Buf;

use super::{FLAG_500, FIN};


// Send the provided file via UDP.
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

      if buf[0] == FLAG_500 {
        return Err(Serr::SERVER(format!("Datastore had unrecoverable error while receving {}", &filename)));
      }

      data_buf.adjust(&buf)?;

      if buf[0] == FIN { return terminate(&data_buf) }
    }
  }
}


/// Terminate connection.
fn terminate(data_buf: &Buf) -> Result<(), Serr> {
  if data_buf.is_done() {
    println!("Sent {} to datastore", data_buf.filename);
    return Ok(());
  }
  Err(Serr::SERVER("Received FIN before all data was sent".to_string()))
}
