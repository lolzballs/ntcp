pub struct ServerSocket {
    interface: SocketInterface,
    tx_socket: mpsc::Sender<Socket>,
}

impl ServerSocket {
    pub fn new(interface: SocketInterface, tx_socket: mpsc::Sender<Socket>) -> Self {
        ServerSocket {
            interface: interface,
            tx_socket: tx_socket,
        }
    }

    pub fn listen(mut self) {
        self.interface.listen(self.tx_socket);
    }
}
