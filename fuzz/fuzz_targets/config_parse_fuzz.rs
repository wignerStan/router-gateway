#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Convert arbitrary bytes to YAML (lossy for non-UTF8).
    // Should never panic during parsing, env expansion, or validation.
    let yaml = String::from_utf8_lossy(data);
    let _ = gateway::config::GatewayConfig::from_yaml(&yaml);
});
