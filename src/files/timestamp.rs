use chrono::prelude::*;

/// Get RFC3339 formatted datetime with timezone and make it filename safe
/// by replacing colons with underscores, e.g.
/// "2018-01-26T18:30:09.453+00:00" => ""2018-01-26T18_30_09.453+00_00".
pub fn fs_timestamp(time: DateTime<Local>) -> String {
    time.to_rfc3339()
        .replace(":", "_")
}
