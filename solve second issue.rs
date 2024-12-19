To handle receiving multiple messages from a client if the socket remains open, you can modify the `handle` method to process messages in a loop. This allows the server to continuously read and respond to messages as long as the connection is active.

Here’s an updated version of your `handle` method:

### Updated `handle` Method

```rust
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
```

---

### Explanation of Changes

1. **Infinite Loop for Continuous Reading**
   - The `loop` construct ensures that the server keeps processing messages as long as the connection is active.

2. **Non-Blocking Mode Handling**
   - If the `stream.read` returns `WouldBlock`, it indicates no data is currently available. The server pauses briefly and retries.

3. **Graceful Disconnection**
   - If `bytes_read` is `0`, it signals that the client has disconnected, and the loop exits.

4. **Error Handling**
   - Errors in reading, writing, or flushing are logged and terminate the connection gracefully.

5. **Immediate Responses**
   - Messages are echoed back immediately after they are decoded and processed.

---

### When to Use This Approach
This method is ideal for scenarios where:
- A client keeps an open socket and sends multiple messages.
- The server needs to respond to each message independently without closing the connection.

### Additional Considerations
1. **Timeouts**: Implement a timeout mechanism to avoid blocking indefinitely if the client becomes unresponsive.
2. **Concurrency**: Ensure that the server can handle multiple clients concurrently by spawning a new thread or using an asynchronous runtime (e.g., `tokio`) for each connection.
3. **Buffer Management**: If messages are larger than 512 bytes, dynamically resize the buffer or use a protocol to handle fragmented messages.