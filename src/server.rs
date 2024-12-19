use crate::message::EchoMessage;
use log::{error, info, warn};
use prost::Message;
use std::{
    io::{self, ErrorKind, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

struct Client {
    stream: TcpStream,
}

impl Client {
    pub fn new(mut stream: TcpStream) -> Self {
        Client { stream }
    }

    pub fn handle(&mut self) -> io::Result<()> {
        let mut buffer = [0; 512];
    
        loop {
            // Read data from the client
            let bytes_read = match self.stream.read(&mut buffer) {
                Ok(bytes) => bytes,
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    // Non-blocking mode: no data available
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(e) => {
                    error!("Error reading from client: {}", e);
                    return Err(e);
                }
            };
    
            // If no bytes are read, the client has disconnected
            if bytes_read == 0 {
                info!("Client disconnected.");
                return Ok(());
            }
    
            // Decode and process the received message
            if let Ok(message) = EchoMessage::decode(&buffer[..bytes_read]) {
                info!("Received: {}", message.content);
    
                // Echo back the message
                let payload = message.encode_to_vec();
                if let Err(e) = self.stream.write_all(&payload) {
                    error!("Failed to send response: {}", e);
                    return Err(e);
                }
    
                // Flush the stream to ensure data is sent immediately
                if let Err(e) = self.stream.flush() {
                    error!("Failed to flush stream: {}", e);
                    return Err(e);
                }
            } else {
                error!("Failed to decode message");
            }
        }
    }    
}

pub struct Server {
    listener: TcpListener,
    is_running: Arc<AtomicBool>,
    port: u16, // Store the dynamically assigned port
    clients: Arc<Mutex<Vec<String>>>, // Track connected clients safely
}

impl Server {
    /// Creates a new server instance
    pub fn new(addr: &str) -> io::Result<Self> {
        let listener = TcpListener::bind(addr)?;

        let local_addr = listener.local_addr()?;
        let port = local_addr.port();
        println!("Server is running on: {}", local_addr);

        let is_running = Arc::new(AtomicBool::new(false));
        let clients = Arc::new(Mutex::new(Vec::new())); // Initializing the client list

        Ok(Server {
            listener,
            is_running,
            port,
            clients,
        })
    }

    // Getter to retrieve the dynamically assigned port
    pub fn get_port(&self) -> u16 {
        self.port
    }

    /// Runs the server, listening for incoming connections and handling them
    pub fn run(&self) -> io::Result<()> {
        self.is_running.store(true, Ordering::SeqCst); // Set the server as running
        info!("Server is running on {}", self.listener.local_addr()?);
    
        // Set the listener to non-blocking mode
        self.listener.set_nonblocking(true)?;
    
        while self.is_running.load(Ordering::SeqCst) {
            match self.listener.accept() {
                Ok((stream, addr)) => {
                    info!("New client connected: {}", addr);

                    // Add the client to the list safely
                    let mut clients = self.clients.lock().unwrap();
                    clients.push(addr.to_string());
                    info!("Connected clients: {:?}", clients);

                    // Spawn a new thread for each client
                    let is_running = Arc::clone(&self.is_running);
                    let clients = Arc::clone(&self.clients);
                    thread::spawn(move || {
                        let mut client = Client::new(stream);
                        while is_running.load(Ordering::SeqCst) {
                            if let Err(e) = client.handle() {
                                error!("Error handling client {}: {}", addr, e);
                                break;
                            }
                        }
                        info!("Client {} disconnected.", addr);

                        // Remove the client from the list safely after disconnection
                        let mut clients = clients.lock().unwrap();
                        if let Some(pos) = clients.iter().position(|x| x == &addr.to_string()) {
                            clients.remove(pos);
                        }
                        info!("Updated connected clients: {:?}", clients);
                    });
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    // No incoming connections, sleep briefly to reduce CPU usage
                    thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    error!("Error accepting connection: {}", e);
                }
            }
        }
    
        info!("Server stopped.");
        Ok(())
    }

    /// Stops the server by setting the `is_running` flag to `false`
    pub fn stop(&self) {
        if self.is_running.load(Ordering::SeqCst) {
            self.is_running.store(false, Ordering::SeqCst);
            // Trigger a shutdown signal to unblock accept()
            if let Ok(_) = TcpStream::connect(format!("127.0.0.1:{}", self.port)) {
                info!("Shutdown signal sent to unblock listener.");
            } else {
                error!("Failed to send shutdown signal.");
            }
        } else {
            warn!("Server was already stopped or not running.");
        }
    }
}
