use stars_formats::StarsFile;
fn main() {
    for p in std::env::args().skip(1) {
        let Ok(b) = std::fs::read(&p) else { continue };
        let Ok(f) = StarsFile::decode(&b) else {
            continue;
        };
        println!("{p}: {:?}", f.latest_segment().header);
    }
}
