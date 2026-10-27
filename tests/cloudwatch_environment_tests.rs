#[path = "support/fake_cloudwatch.rs"]
mod fake_cloudwatch;

use fake_cloudwatch::FakeCloudWatch;
use splash::cloudwatch::{self, Connection, Query};

// The one test in this file sets the process environment, which no other
// test running at the same time may read.
#[test]
fn the_environment_names_the_endpoint_region_and_credentials() {
    let server = FakeCloudWatch::start(&[r#"{"events":[
      {"logStreamName":"web/web/3f9a1c2e","timestamp":1793109601000,"message":"GET /health 200 3ms","eventId":"1"}
    ]}"#]);
    let missing = std::env::temp_dir().join("splash-no-aws-config");

    std::env::set_var("AWS_ENDPOINT_URL", &server.endpoint);
    std::env::set_var("AWS_REGION", "us-east-1");
    std::env::set_var("AWS_ACCESS_KEY_ID", "AKIDEXAMPLE");
    std::env::set_var("AWS_SECRET_ACCESS_KEY", "secret");
    std::env::set_var("AWS_CONFIG_FILE", &missing);
    std::env::set_var("AWS_SHARED_CREDENTIALS_FILE", &missing);
    std::env::set_var("AWS_EC2_METADATA_DISABLED", "true");
    std::env::remove_var("AWS_PROFILE");

    let (lines, reader) = cloudwatch::start(Connection::Environment, Query::new("app-logs"));
    let lines: Vec<String> = lines.iter().map(|received| received.line).collect();

    assert_eq!(
        (lines, reader.join().unwrap(), server.requests()),
        (
            vec!["GET /health 200 3ms".to_string()],
            Ok(()),
            vec![r#"{"logGroupName":"app-logs"}"#.to_string()]
        )
    );
}
