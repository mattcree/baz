//! Generate the spike's test signals into a directory (default: ./fixtures).
//!
//! Usage: gen-signals [OUT_DIR]

use std::path::PathBuf;

use baz_spike_audio_gapless::fixtures;

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("fixtures"), PathBuf::from);
    match fixtures::generate(&dir) {
        Ok(set) => {
            println!("fixtures written to {}", set.dir.display());
            println!("  reference : {}", set.ref_f32.display());
            println!(
                "  halves    : {} / {}",
                set.part1_f32.display(),
                set.part2_f32.display()
            );
            println!(
                "  rate pair : {} / {}",
                set.rate_44k.display(),
                set.rate_48k.display()
            );
            match set.flac {
                Some(f) => println!(
                    "  flac ({}) : {} / {} / {}",
                    f.encoder,
                    f.full.display(),
                    f.part1.display(),
                    f.part2.display()
                ),
                None => println!("  flac      : skipped (no ffmpeg or flac CLI found)"),
            }
        }
        Err(e) => {
            eprintln!("fixture generation failed: {e}");
            std::process::exit(1);
        }
    }
}
