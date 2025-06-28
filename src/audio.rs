use std::io::{self, Write};

pub fn play_beep() {
    // Send terminal bell character to make an audible beep
    print!("\x07");
    io::stdout().flush().unwrap();
}
