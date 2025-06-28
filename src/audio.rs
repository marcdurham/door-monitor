use std::io::{self, Write};

pub fn play_beep() {
    // Send terminal bell character to make an audible beep
    print!("\x07");
    io::stdout().flush().unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_play_beep_does_not_panic() {
        // This test ensures play_beep() can be called without panicking
        // We can't easily test the actual beep sound, but we can ensure it doesn't crash
        play_beep();
        // If we get here, the function didn't panic
        assert!(true);
    }

    #[test]
    fn test_play_beep_multiple_calls() {
        // Test that multiple calls don't cause issues
        for _ in 0..3 {
            play_beep();
        }
        assert!(true);
    }
}
