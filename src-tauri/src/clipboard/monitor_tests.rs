use super::monitor::{monitor_signal, MonitorSignal};

#[test]
fn signals_failing_on_healthy_to_error_edge() {
    assert_eq!(
        MonitorSignal::Failing("boom".to_string()),
        monitor_signal(true, Some("boom"))
    );
}

#[test]
fn signals_recovered_on_error_to_healthy_edge() {
    assert_eq!(MonitorSignal::Recovered, monitor_signal(false, None));
}

#[test]
fn stays_quiet_while_continuously_failing() {
    assert_eq!(MonitorSignal::Quiet, monitor_signal(false, Some("boom")));
}

#[test]
fn stays_quiet_while_continuously_healthy() {
    assert_eq!(MonitorSignal::Quiet, monitor_signal(true, None));
}
