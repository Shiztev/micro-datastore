pub mod server_handle;
pub mod protocol;

use std::{net::{TcpListener, UdpSocket, TcpStream}, io::{Write, BufReader, BufRead, Read, copy}, fs::File};

use protocol::{SLEEP_TIME, LF, CRLF};

/// Length of HTTP version
const HTTP_LEN: usize = 8;

/// ASCII values for HTTP/1.1
const HTTP11: [u8; HTTP_LEN] = [72, 84, 84, 80, 47, 49, 46, 49];

///ASCII values for HTTP/1.0
const HTTP10: [u8; HTTP_LEN] = [72, 84, 84, 80, 47, 49, 46, 48];

/// Length of GET
const LEN_GET: usize = 3;

/// ASCII values for GET request
const GET: [u8; LEN_GET] = [71, 69, 84];

/// Length of POST
const LEN_POST: usize = 4;

/// ASCII values for POST request
const POST: [u8; LEN_POST] = [80, 79, 83, 84];

/// Length of the mime type field Content-Length
const CLEN_LEN: usize = 16;

/// ASCII values for Content-Length: 
const CLEN: [u8; CLEN_LEN] = [67, 111, 110, 116, 101, 110, 116, 45, 76, 101, 110, 103, 116, 104, 58, 32];

/// Minimum Ethernet MTU in bytes
const ETHER_MTU: usize = 1500;

/// UDP header length in bytes
const UDP_H_LEN: usize = 8;

/// IP header length in bytes
const IP_H_LEN: usize = 20;

/// Minimum MTU in bytes
const MTU: usize = ETHER_MTU - UDP_H_LEN - IP_H_LEN;


/// Enum of the possible errors.
#[derive(Debug)]
#[derive(PartialEq)]
pub enum Serr {
  DNE(String),
  SERVER(String),
  NA,
}


/// Enum of the possible errors.
#[derive(Debug)]
#[derive(PartialEq)]
pub enum ReadData {
  MORE,
  DONE,
}


/// An enumeration of supported HTTP operations.
#[derive(Debug)]
#[derive(PartialEq)]
enum Op {
  GET(String),
  POST(String),
  NA,
}


/// Error 404 response
const ERROR_404: &[u8] = "HTTP/1.1 404 NOT FOUND\r\n\r\n".as_bytes();

/// Error 500 response
const ERROR_500: &[u8] = "HTTP/1.1 500 INTERNAL SERVER ERROR\r\n\r\n".as_bytes();


fn main() {
  let args:Vec<String> = std::env::args().collect();
  let ds_addr: String;  // get datastore IP address as command line arg
  let addr: String = "0.0.0.0:40000".to_string();  // listen on all addresses

  if args.len() != 2 {
    panic!("usage: proxy_server <datastore server IP>");
  }
  ds_addr = format!("{}:41000", &args[1]);

  let l: TcpListener = match TcpListener::bind(&addr) {
    Ok(tl) => tl,
    Err(_) => {
      eprintln!("Unable to bind a TCP socket to address");
      return;
    }
  };

  for s in l.incoming() {  // process each request received
    let stream = match s {
      Ok(i) => i,
      Err(_) => {
        eprintln!("Terminating malformed TCP connection");
        continue;
      },
    };

    let socket = match UdpSocket::bind(&addr) {
      Ok(s) => s,
      Err(_) => {
        eprintln!("Unable to bind a UDP socket to address");
        return;
      }
    };
    socket.set_read_timeout(Some(SLEEP_TIME)).expect("System doesn't support set_read_timeout. Please update rust to at least v1.4.0.");
    match socket.connect(&ds_addr) {
      Ok(_) => (),
      Err(_) => {
        eprintln!("Could not connect to datastore address via UDP");
        return;
      }
    }

    handle_error(handle_request(stream, &socket));
  }
}


/// Handles the provided TcpStream that was initialized
/// by a client.
pub fn handle_request(mut stream: TcpStream, socket: &UdpSocket) -> (TcpStream, Result<(), Serr>) {
  let mut reader: BufReader<&mut TcpStream> = BufReader::new(&mut stream);
  let mut buf: Vec<u8> = Vec::new();
  let operation: Op;
  let mut l: usize;
  let mut content_length: u64 = 0;

  // Determine the protocol and data being operated on
  l = read_until_byte(&mut reader, &mut buf, LF);
  if l > 0 {
    operation = determine_protocol(&buf);
  } else {
    return (stream, Result::Err(Serr::NA));
  }

  if operation == Op::NA {
    return (stream, Result::Err(Serr::NA));
  }

  // iterate over lines delinated by line feeds until 
  // buffer only consists of <CR><LF>
  loop {
    // split on line feed
    buf.clear();
    l = read_until_byte(&mut reader, &mut buf, LF);

    // get content length for POST request
    if l >= CLEN_LEN {
      content_length = match fetch_content_len(content_length, &buf) {
        Ok(i) => i,
        Err(e) => return (stream, Result::Err(e)),
      };
    }

    // header fully processed if empty <CR><LF> is read
    if buf == CRLF {
      break;
    }
  }

  let r = match operation {
    Op::GET(fetch_filename) => {
      println!("Receive GET request for {}", fetch_filename);
      server_handle::handle_get(fetch_filename, &stream, socket)
    },
    Op::POST(upload_filename) => {
      println!("Received POST request for {}", upload_filename);
      // read file from stream and save, respective avaiable memory
      let mut file = match File::create(&upload_filename) {
        Ok(f) => f,
        Err(e) => return (stream, Result::Err(Serr::SERVER(format!("Couldn't create file {}:\n{}", &upload_filename, e)))),
      };

      let mut buf: [u8; MTU];
      let mut amt: usize;
      let mut total_read: u64 = 0;
      while total_read != content_length {
        buf = [0; MTU];
        amt = match reader.read(&mut buf) {
          Ok(i) => i,
          Err(e) => return (stream, Result::Err(Serr::SERVER(format!("Couldn't read from stream:\n{}", e)))),
        };

        if (amt == 0) && (total_read != content_length) {
          // ensure total read isn't less than OR greater than content_length
          return (stream, Result::Err(Serr::SERVER(format!("Invalid net amount of data read from stream ({}), should be {} for file {}", total_read, content_length, &upload_filename))));
        }

        match file.write(&buf[0..amt]) {
          Ok(_) => (),
          Err(e) => return (stream, Result::Err(Serr::SERVER(format!("Couldn't save buf to {}:\n{}", &upload_filename, e)))),
        }
        total_read += amt as u64;
      }

      server_handle::handle_post(upload_filename, content_length, &stream, socket)
    },
    Op::NA => Result::Err(Serr::NA),
  };

  match r {
    Ok(_) => (stream, Result::Ok(())),
    Err(e) => (stream, Result::Err(e)),
  }
}


/// Fetch the value stored in the Content-Length field of the HTTP header,
/// if the buffer is the Content-Length field.
fn fetch_content_len(curr_cl: u64, buf: &Vec<u8>) -> Result<u64, Serr> {
  if buf[0..CLEN_LEN] == CLEN {
    return
    match bytes_to_str(buf, CLEN_LEN, buf.len() - 2)  // account for trailing <CR><LF>
    .parse::<u64>()
    {
      Ok(i) => Result::Ok(i),
      Err(_) => {
        return Result::Err(Serr::SERVER("Invalid Content-Length received, terminating TCP stream.".to_string()));
      },
    };

  } else {
    return Result::Ok(curr_cl);
  }
}


fn read_until_byte<T: std::io::Read>(reader: &mut BufReader<&mut T>, buf: &mut Vec<u8>, byte: u8) -> usize {
  match reader.read_until(byte, buf) {
    Ok(i) => i,
    Err(_) => 0,
  }
}


/// Determines the HTTP protocol in use, if any.
fn determine_protocol(data: &Vec<u8>) -> Op {
  let end: usize = data.len() - 2;
  let p: &[u8] = &data[end - HTTP_LEN..end];
  let path_end = end - HTTP_LEN - 1;  // Remove the space between path and HTTP version

  // check end is valid http version: http/1.0 or http/1.1
  if (p != HTTP10) && (p != HTTP11) {
    return Op::NA;
  }

  if data[0..LEN_GET] == GET {
    return Op::GET(format!(".{}", bytes_to_str(data, LEN_GET + 1, path_end)));

  } else if data[0..LEN_POST] == POST {
    return Op::POST(format!(".{}", bytes_to_str(data, LEN_POST + 1, path_end)));

  } else {
    return Op::NA;
  }
}


/// Creates a string from a designated slice of a byte buffer.
fn bytes_to_str(buf: &[u8], start: usize, end: usize) -> String {
  buf[start..end]
    .into_iter()
    .map(|b| *b as char)
    .collect::<String>()
}


/// Send an error to the proxy if an error occurs.
fn handle_error(result: (TcpStream, Result<(), Serr>)) {
  match result.1 {
    Ok(_) => (),
    Err(e) => send_error(&result.0, e),
  }
}


/// Send the respective error for the server error over the
/// provided socket.
fn send_error(stream: &TcpStream, serr: Serr) {
  let err_msg: String = match serr {
    Serr::DNE(e) => { send_404_error(stream); e},
    Serr::SERVER(e) => { send_500_error(stream); e},
    Serr::NA => format!("Unsupported request received."),
  };
  eprintln!("{}", err_msg);
}


/// Send an HTTP Error 404 over the provided stream.
fn send_404_error(stream: &TcpStream) {
  respond(ERROR_404, stream, "Interrupted while sending 404 response");
}


/// Send an HTTP Error 500 over the provided stream.
fn send_500_error(stream: &TcpStream) {
  respond(ERROR_500, stream, "Interrupted while sending 500 response");
}


/// Send bytes over a stream, and print the provided error msg to stderr if
/// an error occurs.
fn respond(buf: &[u8], mut stream: &TcpStream, err_msg: &str) {
  match stream.write_all(buf) {
    Ok(_) => (),
    Err(_) => eprintln!("{}", err_msg),
  }
}
