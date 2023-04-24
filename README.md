# About

A datastore service that supports HTTP POST and GET requests through a unique protocol.

This project provides a web service to store data remotely. By hosting a proxy which mediates communication between clients and a remote datastore, the service allows users to upload and read files on said datastore server.

The most notable feature of this project is an implementation of a reliable UDP based protocol, which works over networks with MTU's no less than Ethernet's 1500 byte MTU. This protocol is used to transfer data between the proxy and datastore.

Only the standard Rust library was used in this projects development.

### Side note:

This project can only handle sequential requests. The server's are currently unthreaded, and making multiple requests at once will break the service.

## Prerequisite:

- Have the Rust language and Cargo installed (This program was developed on cargo/rustc version 1.64.0)
- Ensure that ports 40000 and 41000 are not blocked on the devices you plan to run the servers on.

## Requirements:

- You must be able to identify the IP on the machine you run the datastore code on.

## Note:

In this repository, there are two directories: proxy_server
and datastore_server. 
- proxy_server corresponds to the client facing server.
- datastore_server corresponds to the datastore that said client facing server communicates with.

# How to run:

1) Clone the repository.

2) In the cloned directory, there will be a proxy_server
    folder and a datastore_server folder. Place each of these on the device/location that you plan to run them from. <br /><br /> *Take special note* of the devices/virtual environments you plan to run each server on. You will need to know the ***STATIC*** IP address of the device you run the datastore server on (the proxy server assumes that the IP is static). <br />The IP address of the proxy server's device will be required to make requests, in case it is being run remotely of where you're making requests.

3) From the datastore_server directory, run `cargo run` via a terminal.

4) From the proxy_server directory, run `cargo run <datastore-server-IP>`, where <datastore-server-IP> is the IP of the device that is running the datastore server.

5) You can now make HTTP GET and POST requests to the IP of the proxy server's device.
