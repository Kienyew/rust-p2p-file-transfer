pub mod peer;
pub mod torrent;
pub mod tracker;

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};

use byteorder::{NetworkEndian, ReadBytesExt, WriteBytesExt};
use bytes::{Bytes, BytesMut};
use prost::Message;

pub mod requests {
    include!(concat!(env!("OUT_DIR"), "/requests.rs"));
}

pub mod responses {
    include!(concat!(env!("OUT_DIR"), "/responses.rs"));
}

use requests::request;
use requests::Request;
use responses::response;
use responses::Response;

type ChunkId = u64;
pub const CHUNK_SIZE: u64 = 262144;

fn get_stream_message_length(stream: &mut TcpStream) -> io::Result<u64> {
    Ok(stream.read_u64::<NetworkEndian>()?)
}

pub fn read_request(stream: &mut TcpStream) -> io::Result<Request> {
    let message_length = get_stream_message_length(stream)?;
    let mut buffer = vec![0; message_length as usize];

    stream
        .read_exact(&mut buffer[..])
        .expect("stream read error");
    let buffer = Bytes::from(buffer);
    let request = Request::decode(buffer).expect("stream bad request message format");
    Ok(request)
}

pub fn get_join_request(listening_addr: SocketAddr) -> Request {
    let mut request = Request::default();
    request.r#type = Some(request::Type::Join(request::Join {
        listening_addr: listening_addr.to_string(),
    }));
    request
}

pub fn get_active_proof_request(addr: SocketAddr) -> Request {
    let mut request = Request::default();
    request.r#type = Some(request::Type::ActiveProof(request::ActiveProof {
        listening_addr: addr.to_string(),
    }));
    request
}

pub fn get_peer_list_request() -> Request {
    let mut request = Request::default();
    request.r#type = Some(request::Type::PeerList(request::PeerList {}));
    request
}

pub fn get_chunks_query_request() -> Request {
    let mut request = Request::default();
    request.r#type = Some(request::Type::ChunksQuery(request::ChunksQuery {}));
    request
}

pub fn get_fetch_chunk_request(chunk_id: ChunkId) -> Request {
    let mut request = Request::default();
    request.r#type = Some(request::Type::FetchChunk(request::FetchChunk { chunk_id }));
    request
}

pub fn get_ok_response() -> Response {
    let mut response = Response::default();
    response.r#type = Some(response::Type::Ok(response::Ok {}));
    response
}

pub fn get_bad_response() -> Response {
    let mut response = Response::default();
    response.r#type = Some(response::Type::Bad(response::Bad {}));
    response
}

pub fn get_chunks_query_response(chunk_ids: Vec<ChunkId>) -> Response {
    let mut response = Response::default();
    response.r#type = Some(response::Type::ChunksQuery(response::ChunksQuery {
        chunk_ids,
    }));

    response
}

pub fn get_fetch_chunk_response(chunk: Vec<u8>) -> Response {
    let mut response = Response::default();
    response.r#type = Some(response::Type::Chunk(chunk));
    response
}

pub fn read_response(stream: &mut TcpStream) -> io::Result<Response> {
    let message_length = get_stream_message_length(stream)?;
    let mut buffer = vec![0; message_length as usize];

    stream
        .read_exact(&mut buffer[..])
        .expect("stream read error");
    let buffer = Bytes::from(buffer);
    let response = Response::decode(buffer).expect("stream bad response message format");
    Ok(response)
}

pub fn read_peer_list_response(stream: &mut TcpStream) -> io::Result<Vec<SocketAddr>> {
    let response = read_response(stream)?;
    if let Some(response::Type::PeerList(peers)) = response.r#type {
        Ok(peers
            .addresses
            .into_iter()
            .map(|s| s.parse().unwrap())
            .collect())
    } else {
        panic!("bad peer list response from tracker");
    }
}

pub fn read_chunks_query_response(stream: &mut TcpStream) -> io::Result<Vec<ChunkId>> {
    if let Some(response::Type::ChunksQuery(chunks)) = read_response(stream)?.r#type {
        return Ok(chunks.chunk_ids);
    }

    panic!("we don't handle unexpecting error");
}

pub fn read_fetch_chunk_response(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    if let Some(response::Type::Chunk(chunk)) = read_response(stream)?.r#type {
        return Ok(chunk);
    }
    panic!("we don't handle unexpecting error");
}

fn send_message<T: Message>(stream: &mut TcpStream, message: T) -> io::Result<()> {
    let mut message_bytes = BytesMut::new();
    message.encode(&mut message_bytes).unwrap();
    stream.write_u64::<NetworkEndian>(message_bytes.len() as u64)?;
    stream.write_all(message_bytes.as_ref())?;

    Ok(())
}
