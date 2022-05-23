use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::torrent::Torrent;

use threadpool::ThreadPool;

type ChunkId = u64;

pub struct Peer {
    addr: SocketAddr,
    torrent: Torrent,
    thread_pool: ThreadPool,
    neighbors: Arc<Mutex<HashMap<SocketAddr, Vec<ChunkId>>>>,
    downloaded_chunks: Arc<Mutex<Vec<ChunkId>>>,
    file: Arc<Mutex<File>>,
}

impl Peer {
    pub fn as_peer(addr: SocketAddr, torrent: Torrent, file_name: &Path) -> Self {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .truncate(true)
            .create(true)
            .open(file_name)
            .expect("open file error");

        file.set_len(torrent.file_size).unwrap();

        Peer {
            addr,
            file: Arc::new(Mutex::new(file)),
            torrent,
            thread_pool: ThreadPool::new(8),
            neighbors: Arc::new(Mutex::new(HashMap::new())),
            downloaded_chunks: Arc::new(Mutex::new(vec![])),
        }
    }

    pub fn as_seeder(addr: SocketAddr, torrent: Torrent, file_name: &Path) -> Self {
        use crate::CHUNK_SIZE;

        let file = OpenOptions::new()
            .read(true)
            .open(file_name)
            .expect("open file error");

        let mut downloaded_chunks = vec![];
        let mut chunk_id = 0;
        while chunk_id < torrent.file_size {
            downloaded_chunks.push(chunk_id);
            chunk_id += CHUNK_SIZE;
        }

        Peer {
            addr,
            file: Arc::new(Mutex::new(file)),
            torrent,
            thread_pool: ThreadPool::new(8),
            neighbors: Arc::new(Mutex::new(HashMap::new())),
            downloaded_chunks: Arc::new(Mutex::new(downloaded_chunks)),
        }
    }

    pub fn start(&mut self) {
        let listener = TcpListener::bind(self.addr).expect("tcp listener bind error");
        println!("Start listening at {}", self.addr);
        self.join_the_swarm(listener.local_addr().unwrap());

        let tracker_addr = self.torrent.tracker_addr;
        let listening_addr = self.addr;
        std::thread::spawn(move || active_proof_loop(tracker_addr, listening_addr));
        let neighbors = Arc::clone(&self.neighbors);
        std::thread::spawn(move || update_neighbors_loop(tracker_addr, listening_addr, neighbors));
        let neighbors = Arc::clone(&self.neighbors);
        std::thread::spawn(move || update_downloaded_chunks_loop(neighbors));

        for _ in 0..8 {
            let neighbors = Arc::clone(&self.neighbors);
            let downloaded_chunks = Arc::clone(&self.downloaded_chunks);
            let file = Arc::clone(&self.file);
            let file_size = self.torrent.file_size;
            self.thread_pool
                .execute(move || fetch_chunk_loop(neighbors, downloaded_chunks, file, file_size));
        }

        for stream in listener.incoming().filter_map(|x| x.ok()) {
            self.handle_peer(stream);
        }
    }

    fn join_the_swarm(&mut self, listening_addr: SocketAddr) {
        println!("Attempt to join the swarm");
        let mut stream =
            TcpStream::connect(self.torrent.tracker_addr).expect("stream connect tracker error");
        let message = crate::get_join_request(listening_addr);
        crate::send_message(&mut stream, message).unwrap();
        crate::read_response(&mut stream).unwrap();
    }

    fn get_local_chunk(&self, chunk_id: ChunkId) -> Vec<u8> {
        use crate::CHUNK_SIZE;
        use std::cmp::min;

        let start_position = chunk_id as usize;
        let end_position = min(
            start_position + CHUNK_SIZE as usize,
            self.torrent.file_size as usize,
        );
        let file = self.file.lock().unwrap();
        let map = unsafe { memmap::Mmap::map(&file).unwrap() };
        (&map[start_position..end_position])
            .iter()
            .cloned()
            .collect()
    }

    fn handle_peer(&mut self, mut stream: TcpStream) {
        use crate::requests::request::Type;
        let request = if let Ok(request) = crate::read_request(&mut stream) {
            request
        } else {
            return;
        };

        match request.r#type.unwrap() {
            Type::ChunksQuery(_) => self.handle_chunks_query_request(&mut stream),
            Type::FetchChunk(chunk_id) => {
                self.handle_fetch_chunk_request(&mut stream, chunk_id.chunk_id)
            }
            _ => {}
        }
    }

    fn handle_chunks_query_request(&mut self, stream: &mut TcpStream) {
        use rand::seq::SliceRandom;
        use rand::thread_rng;
        let mut rng = thread_rng();
        self.downloaded_chunks.lock().unwrap().shuffle(&mut rng);
        let response =
            crate::get_chunks_query_response(self.downloaded_chunks.lock().unwrap().clone());
        crate::send_message(stream, response).ok();
    }

    fn handle_fetch_chunk_request(&mut self, stream: &mut TcpStream, chunk_id: ChunkId) {
        let chunk = self.get_local_chunk(chunk_id);
        let response = crate::get_fetch_chunk_response(chunk);
        crate::send_message(stream, response).ok();
    }
}

fn active_proof_loop(tracker_addr: SocketAddr, listening_addr: SocketAddr) {
    loop {
        let mut stream = TcpStream::connect(tracker_addr).expect("stream connect tracker error");
        let request = crate::get_active_proof_request(listening_addr);
        crate::send_message(&mut stream, request).unwrap();
        std::thread::sleep(Duration::from_millis(2500));
    }
}

fn update_neighbors_loop(
    tracker_addr: SocketAddr,
    self_addr: SocketAddr,
    neighbors: Arc<Mutex<HashMap<SocketAddr, Vec<ChunkId>>>>,
) {
    loop {
        {
            println!("Updating neighbors list");
            let mut stream =
                TcpStream::connect(tracker_addr).expect("stream connect tracker error");
            let request = crate::get_peer_list_request();
            crate::send_message(&mut stream, request).unwrap();
            let peers = crate::read_peer_list_response(&mut stream).unwrap();
            let mut neighbors = neighbors.lock().unwrap();
            neighbors.retain(|neighbor, _| {
                if !peers.contains(neighbor) {
                    println!("Dropping {} because he no longer in peer list", neighbor);
                }
                peers.contains(neighbor)
            });
            for peer in peers {
                if !neighbors.contains_key(&peer) && peer != self_addr {
                    println!("Adding new neighbor: {}", peer);
                    neighbors.insert(peer, vec![]);
                }
            }
        }

        std::thread::sleep(Duration::from_millis(2500));
    }
}

fn update_downloaded_chunks_loop(neighbors: Arc<Mutex<HashMap<SocketAddr, Vec<ChunkId>>>>) {
    loop {
        {
            let neighbor_addrs = neighbors
                .lock()
                .unwrap()
                .keys()
                .cloned()
                .collect::<Vec<SocketAddr>>();

            for neighbor in neighbor_addrs {
                println!("Getting downloaded chunks of neighbor: {}", neighbor);
                let mut stream = if let Ok(s) = TcpStream::connect(neighbor) {
                    s
                } else {
                    println!("Dropping neighbor: {}", neighbor);
                    neighbors.lock().unwrap().remove(&neighbor);
                    continue;
                };

                let request = crate::get_chunks_query_request();
                if crate::send_message(&mut stream, request).is_err() {
                    println!("Dropping neighbor: {}", neighbor);
                    neighbors.lock().unwrap().remove(&neighbor);
                    continue;
                }

                if let Ok(chunks) = crate::read_chunks_query_response(&mut stream) {
                    println!("neighbor {} having {} chunks", neighbor, chunks.len());
                    neighbors
                        .lock()
                        .unwrap()
                        .entry(neighbor)
                        .and_modify(|v| *v = chunks);
                } else {
                    println!("Dropping neighbor: {}", neighbor);
                    neighbors.lock().unwrap().remove(&neighbor);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(1000));
    }
}

// select a chunk I doesn't have, and fetch a random neighbor with that chunk
fn fetch_chunk_loop(
    neighbors: Arc<Mutex<HashMap<SocketAddr, Vec<ChunkId>>>>,
    downloaded_chunks: Arc<Mutex<Vec<ChunkId>>>,
    file: Arc<Mutex<File>>,
    file_size: u64,
) {
    loop {
        let found;
        {
            let target = {
                use rand::seq::SliceRandom;
                use rand::thread_rng;
                let mut rng = thread_rng();
                let neighbors_guard = neighbors.lock().unwrap();
                let mut neighbor_addrs: Vec<SocketAddr> = neighbors_guard.keys().cloned().collect();
                neighbor_addrs.shuffle(&mut rng);
                let downloaded_chunks = downloaded_chunks.lock().unwrap();
                let mut target: Option<(SocketAddr, ChunkId)> = None;
                for neighbor_addr in neighbor_addrs {
                    let chunk_id = neighbors_guard
                        .get(&neighbor_addr)
                        .unwrap()
                        .iter()
                        .find(|chunk_id| !downloaded_chunks.contains(chunk_id));
                    if let Some(chunk_id) = chunk_id {
                        target = Some((neighbor_addr, *chunk_id));
                        break;
                    }
                }
                target
            };

            if let Some((neighbor, chunk_id)) = target {
                found = true;
                fetch_chunk_from_neighbor(
                    neighbor,
                    chunk_id,
                    Arc::clone(&neighbors),
                    Arc::clone(&downloaded_chunks),
                    Arc::clone(&file),
                    file_size,
                );
            } else {
                found = false;
            }
        }

        if !found {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

fn fetch_chunk_from_neighbor(
    neighbor: SocketAddr,
    chunk_id: ChunkId,
    neighbors: Arc<Mutex<HashMap<SocketAddr, Vec<ChunkId>>>>,
    downloaded_chunks: Arc<Mutex<Vec<ChunkId>>>,
    file: Arc<Mutex<File>>,
    file_size: u64,
) {
    println!(
        "Attempt to fetch chunk {} from neighbor {}",
        chunk_id, neighbor
    );
    let mut stream = if let Ok(stream) = TcpStream::connect(neighbor) {
        stream
    } else {
        println!("Dropping neighbor: {}", neighbor);
        neighbors.lock().unwrap().remove(&neighbor);
        return;
    };

    let request = crate::get_fetch_chunk_request(chunk_id);
    if crate::send_message(&mut stream, request).is_err() {
        println!("Dropping neighbor: {}", neighbor);
        neighbors.lock().unwrap().remove(&neighbor);
        return;
    }
    if let Ok(chunk) = crate::read_fetch_chunk_response(&mut stream) {
        let mut downloaded_chunks = downloaded_chunks.lock().unwrap();
        // avoid duplicates chunk due to multithreading
        if !downloaded_chunks.contains(&chunk_id) {
            downloaded_chunks.push(chunk_id);
            write_chunk_to_local(chunk_id, chunk, file, file_size);
        }
    } else {
        println!("Dropping neighbor: {}", neighbor);
        neighbors.lock().unwrap().remove(&neighbor);
    }
}

fn write_chunk_to_local(chunk_id: ChunkId, chunk: Vec<u8>, file: Arc<Mutex<File>>, file_size: u64) {
    println!("Writing chunk {} to local file system", chunk_id);
    use crate::CHUNK_SIZE;
    use std::cmp::min;

    let start_position = chunk_id as usize;
    let end_position = min(start_position + CHUNK_SIZE as usize, file_size as usize);
    let file = file.lock().unwrap();
    let mut map = unsafe { memmap::MmapMut::map_mut(&file).unwrap() };
    (&mut map[start_position..end_position])
        .write(&chunk[..])
        .unwrap();
}
