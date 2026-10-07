//! Serial output scanning (task C01_45): detect the expected text or the failure
//! marker in collected serial bytes. Pure helpers, no I/O.

/// Failure marker printed by Blargg test ROMs on failure (project decision; D_17).
pub const FAIL_MARKER: &str = "Failed";

/// Result of scanning a chunk of serial output against the expectation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scan {
    /// Neither the expected text nor the failure marker is present yet.
    Continue,
    /// The expected text was found in the collected serial output.
    Found,
    /// The failure marker was found in the collected serial output.
    Failed,
}

/// Append `new_bytes` to `collected` and report whether it now contains the expected
/// text or the failure marker (task C01_45).
pub fn scan_serial(collected: &mut String, new_bytes: &[u8], expect: Option<&str>) -> Scan {
    collected.push_str(&String::from_utf8_lossy(new_bytes));
    if let Some(text) = expect {
        if collected.contains(text) {
            return Scan::Found;
        }
    }
    if collected.contains(FAIL_MARKER) {
        return Scan::Failed;
    }
    Scan::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c01_45_scan_continue_on_plain_text() {
        let mut collected = String::new();
        assert_eq!(scan_serial(&mut collected, b"abc", None), Scan::Continue);
        assert_eq!(collected, "abc");
    }

    #[test]
    fn c01_45_scan_found_when_expectation_split_across_calls() {
        let mut collected = String::new();
        assert_eq!(
            scan_serial(&mut collected, b"Pa", Some("Passed")),
            Scan::Continue
        );
        assert_eq!(
            scan_serial(&mut collected, b"ssed", Some("Passed")),
            Scan::Found
        );
    }

    #[test]
    fn c01_45_scan_failed_on_failure_marker() {
        let mut collected = String::new();
        assert_eq!(
            scan_serial(&mut collected, b"Failed #3", None),
            Scan::Failed
        );
    }
}
