#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Convert arbitrary bytes to a string (lossy for non-UTF8).
    // Should never panic regardless of input.
    let url = String::from_utf8_lossy(data);
    let _ = gateway::utils::ssrf::validate_url_not_private(&url);
});
