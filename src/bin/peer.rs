use p2p::peer::Peer;
use p2p::torrent::Torrent;
use std::net::SocketAddr;
use std::path::Path;

use clap::{App, Arg};

fn main() {
    let app = App::new("peer")
        .about("run a peer")
        .arg(
            Arg::with_name("host")
                .help("address to listen on")
                .value_name("ip:port")
                .required(true),
        )
        .arg(
            Arg::with_name("torrent")
                .help("path of the torrent file")
                .required(true),
        )
        .arg(
            Arg::with_name("file_name")
                .help("output file name")
                .required(true),
        )
        .arg(
            Arg::with_name("role")
                .help("initially run as a normal peer or seeder")
                .required(true)
                .possible_values(&["peer", "seeder"]),
        );

    let matches = app.get_matches();
    let listening_addr: SocketAddr = matches.value_of("host").unwrap().parse().unwrap();
    let torrent = Torrent::from_file(Path::new(matches.value_of("torrent").unwrap()));
    let file_name = matches.value_of("file_name").unwrap();
    let role = matches.value_of("role").unwrap();
    let mut peer = match role {
        "peer" => Peer::as_peer(listening_addr, torrent, Path::new(file_name)),
        "seeder" => Peer::as_seeder(listening_addr, torrent, Path::new(file_name)),
        _ => return,
    };
    peer.start();
}
