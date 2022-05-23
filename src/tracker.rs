use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use threadpool::ThreadPool;

use crate::responses;

pub struct Tracker {
    client_expire_times: Arc<Mutex<HashMap<SocketAddr, SystemTime>>>,
    thread_pool: ThreadPool,
    read_timeout: Duration,
}

const EXPIRE_SECONDS: f64 = 5.0;

impl Tracker {
    pub fn new() -> Self {
        let thread_pool = ThreadPool::new(32);
        Tracker {
            client_expire_times: Arc::new(Mutex::new(HashMap::new())),
            thread_pool,
            read_timeout: Duration::from_secs(1),
        }
    }

    pub fn start(&mut self, socket_addr: SocketAddr) {
        let client_expire_times = Arc::clone(&self.client_expire_times);
        std::thread::spawn(|| check_expire_loop(client_expire_times));

        let listener = TcpListener::bind(socket_addr)
            .expect(format!("listener cannot bind at {}", socket_addr).as_str());

        println!("Tracker listening on {}", socket_addr);

        for stream in listener.incoming() {
            let stream = stream.expect("incoming stream error");
            println!("Incoming connection from {}", stream.peer_addr().unwrap());
            stream
                .set_read_timeout(Some(self.read_timeout))
                .expect("stream set read timeout error");
            self.handle_client(stream);
        }
    }

    fn handle_client(&mut self, mut stream: TcpStream) {
        use crate::requests::request::Type;
        let request = if let Ok(request) = crate::read_request(&mut stream) {
            request
        } else {
            return;
        };

        match request.r#type.unwrap() {
            Type::Join(client) => self
                .handle_peer_joining_request(&mut stream, client.listening_addr.parse().unwrap()),
            Type::ActiveProof(client) => self
                .handle_active_proof_request(&mut stream, client.listening_addr.parse().unwrap()),
            Type::PeerList(_) => self.handle_peer_list_request(&mut stream),
            _ => {}
        }
    }

    fn handle_peer_joining_request(
        &mut self,
        stream: &mut TcpStream,
        client_listening_addr: SocketAddr,
    ) {
        println!(
            "Handling peer join request from {}, he is listening at {}",
            stream.peer_addr().unwrap(),
            client_listening_addr
        );
        self.client_expire_times
            .lock()
            .unwrap()
            .insert(client_listening_addr, SystemTime::now());
        crate::send_message(stream, crate::get_ok_response());
    }

    fn handle_active_proof_request(
        &mut self,
        stream: &mut TcpStream,
        client_listening_addr: SocketAddr,
    ) {
        println!(
            "handling active proof request from {}, client listening at {}",
            stream.peer_addr().unwrap(),
            client_listening_addr
        );
        self.client_expire_times
            .lock()
            .unwrap()
            .insert(client_listening_addr, SystemTime::now());
        crate::send_message(stream, crate::get_ok_response());
    }

    fn handle_peer_list_request(&mut self, stream: &mut TcpStream) {
        println!(
            "handling peer list request from {}",
            stream.peer_addr().unwrap()
        );
        let response = self.get_peer_list_response();
        crate::send_message(stream, response);
    }

    fn get_peer_list_response(&self) -> responses::Response {
        use responses::response;
        use responses::response::Type;
        let mut response = responses::Response::default();
        let addresses = self
            .client_expire_times
            .lock()
            .unwrap()
            .keys()
            .map(|peer_addr| peer_addr.to_string())
            .collect();

        response.r#type = Some(Type::PeerList(response::PeerList { addresses }));
        response
    }
}

fn check_expire_loop(client_expire_times: Arc<Mutex<HashMap<SocketAddr, SystemTime>>>) {
    loop {
        client_expire_times
            .lock()
            .unwrap()
            .retain(|addr, expire_time| {
                if !(expire_time.elapsed().unwrap().as_secs_f64() < EXPIRE_SECONDS) {
                    println!("{} expire, dropping it", addr);
                }
                expire_time.elapsed().unwrap().as_secs_f64() < EXPIRE_SECONDS
            });
        std::thread::sleep(Duration::from_millis(500));
    }
}
