use bcsp_contracts::{SERVICE_STATUS_V2_CONTRACT_VERSION, ServiceStatusV2};

#[test]
fn service_status_v2_golden_is_exact_and_round_trips() {
    let golden = include_str!("golden/service-status-v2.json");
    let status = serde_json::from_str::<ServiceStatusV2>(golden).expect("valid V2 status golden");
    assert_eq!(status.contract_version, SERVICE_STATUS_V2_CONTRACT_VERSION);
    assert_eq!(
        format!(
            "{}\n",
            serde_json::to_string_pretty(&status).expect("serialize V2 status")
        ),
        golden.replace("\r\n", "\n")
    );
}
