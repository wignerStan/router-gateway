#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let token = String::from_utf8_lossy(data);

    // Should never panic with arbitrary token against empty list
    let _ = gateway::utils::security::constant_time_token_matches(&token, &[]);

    // Also test with the token itself in the list
    let list = vec![token.to_string()];
    let _ = gateway::utils::security::constant_time_token_matches(&token, &list);

    // Test against a list with multiple entries including the token
    let multi = vec!["other-token".to_string(), token.to_string(), "third".to_string()];
    let _ = gateway::utils::security::constant_time_token_matches(&token, &multi);
});
