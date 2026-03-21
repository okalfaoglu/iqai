//! IQAI Web - chart annotations and Elliott Wave scanning

pub mod api_json;
pub mod api_spec;
pub mod ai;
pub mod chart_data;
pub mod http_app;
pub mod notify;

pub use http_app::{build_router, run_server};
pub mod q_analiz_card;
pub mod q_setup_card;
pub mod trade_open_card;
pub mod trade_close_card;

#[cfg(test)]
mod openapi_spec_tests {
    #[test]
    fn openapi_yaml_exists_and_valid_prefix() {
        let y = include_str!("../openapi.yaml");
        assert!(y.contains("openapi:"));
        assert!(y.contains("paths:"));
    }
}
