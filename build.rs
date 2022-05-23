fn main() {
    prost_build::compile_protos(&["src/requests.proto", "src/responses.proto"], &["src/"]).unwrap();
}

