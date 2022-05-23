use clap::{App, Arg};
use p2p::tracker::Tracker;

fn main() {
    let app = App::new("tracker").about("start a tracker").arg(
        Arg::with_name("host")
            .help("address to listen on")
            .value_name("IP:port")
            .required(true),
    );

    let matches = app.get_matches();
    let addr = matches.value_of("host").unwrap().parse().unwrap();
    let mut tracker = Tracker::new();
    tracker.start(addr);
}
