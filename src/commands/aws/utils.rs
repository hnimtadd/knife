use aws_sdk_sts::primitives::DateTime as AwsDateTime;
use chrono::{DateTime, Local, TimeZone, Utc};

pub fn aws_datetime_to_local(aws_dt: &AwsDateTime) -> DateTime<Local> {
    let utc_datetime = Utc
        .timestamp_opt(aws_dt.secs(), aws_dt.subsec_nanos())
        .unwrap();
    utc_datetime.with_timezone(&Local)
}
