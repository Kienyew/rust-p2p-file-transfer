use std::net::SocketAddr;
use std::path::Path;

use serde_json::Value;

pub struct Torrent {
    pub file_size: u64,
    pub tracker_addr: SocketAddr,
}

impl Torrent {
    pub fn from_file(path: &Path) -> Self {
        let values: Value =
            serde_json::from_str(std::fs::read_to_string(path).unwrap().as_str()).unwrap();
        let values = values.as_object().unwrap();
        let file_size = values.get("file_size").unwrap().as_u64().unwrap();
        let tracker_addr = values
            .get("tracker_addr")
            .unwrap()
            .as_str()
            .unwrap()
            .parse()
            .unwrap();

        Torrent {
            file_size,
            tracker_addr,
        }
    }
}
