pub struct Config {
    /// Whether peers are visible by default.
    pub default_visibility: bool,

    /// Protocol names
    pub protocols: Vec<String>,

    /// Max results
    pub max_results: usize,

    /// Max payload size for requests
    pub request_max_payload_size: usize,
    /// Max payload size for responses
    pub response_max_payload_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            default_visibility: false,
            protocols: vec!["/mubelotix-discovery/0.1.0".to_string()],
            max_results: 100,
            request_max_payload_size: 50_000,
            response_max_payload_size: 500_000,
        }
    }
}
