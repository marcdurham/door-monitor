use std::time::Duration;

pub fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let days = total_seconds / 86400;
    let hours = (total_seconds % 86400) / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    
    if days > 0 {
        format!("{}d {:02}:{:02}:{:02}", days, hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_format_duration_seconds() {
        let duration = Duration::from_secs(45);
        assert_eq!(format_duration(duration), "00:00:45");
    }

    #[test]
    fn test_format_duration_minutes() {
        let duration = Duration::from_secs(5 * 60 + 30);
        assert_eq!(format_duration(duration), "00:05:30");
    }

    #[test]
    fn test_format_duration_hours() {
        let duration = Duration::from_secs(2 * 3600 + 15 * 60 + 45);
        assert_eq!(format_duration(duration), "02:15:45");
    }

    #[test]
    fn test_format_duration_days() {
        let duration = Duration::from_secs(2 * 86400 + 3 * 3600 + 20 * 60 + 15);
        assert_eq!(format_duration(duration), "2d 03:20:15");
    }

    #[test]
    fn test_format_duration_one_day() {
        let duration = Duration::from_secs(86400); // Exactly 1 day
        assert_eq!(format_duration(duration), "1d 00:00:00");
    }

    #[test]
    fn test_format_duration_zero() {
        let duration = Duration::from_secs(0);
        assert_eq!(format_duration(duration), "00:00:00");
    }

    #[test]
    fn test_format_duration_large() {
        let duration = Duration::from_secs(365 * 86400 + 12 * 3600 + 30 * 60 + 45);
        assert_eq!(format_duration(duration), "365d 12:30:45");
    }
}
