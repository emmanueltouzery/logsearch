// I tried https://docs.rs/crate/dtparse/1.0.3 but it didn't work well enough
// for me, plus it doesn't return a format string
use chrono::prelude::*;
use regex::Regex;

const KNOWN_FORMATS: &[(&'static str, fn(&str) -> DateTime<Utc>)] = &[
    // "23-Apr-2020 00:00:00.001" -- tomee log format
    (r"[0-2]\d-\w{3}-\d{4} \d\d:\d\d:\d\d\.\d{3}", parse_tomee),
    // "Apr 26 10:05:02" -- journalctl
    (r"\w{3} [0-2]\d \d\d:\d\d:\d\d", parse_journalctl),
    // 10/Oct/2000:13:55:36 -0700 -- apache
    (
        r"[0-2]\d/\w{3}/\d{4}:\d\d:\d\d:\d\d [+-]\d{4}",
        parse_apache,
    ),
    // 2014-11-12 16:28:21.700 MST -- postgres
    (r"\d{4}-\d\d-[0-2]\d \d\d:\d\d:\d\d\.\d{3}", parse_postgres),
];

fn parse_tomee(str: &str) -> DateTime<Utc> {
    Utc.from_utc_datetime(
        &NaiveDateTime::parse_from_str(str, "%d-%b-%Y %T%.3f")
            .ok()
            .unwrap(),
    )
}

fn parse_journalctl(str: &str) -> DateTime<Utc> {
    // the year is missing... hopefully there's a better way, but for now...
    Utc.from_utc_datetime(
        &NaiveDateTime::parse_from_str(&format!("{}-{}", Local::now().year(), str), "%Y-%b %d %T")
            .ok()
            .unwrap(),
    )
}

fn parse_apache(str: &str) -> DateTime<Utc> {
    DateTime::parse_from_str(str, "%d/%b/%Y:%T %z")
        .ok()
        .unwrap()
        .into()
}

fn parse_postgres(str: &str) -> DateTime<Utc> {
    Utc.from_utc_datetime(
        &NaiveDateTime::parse_from_str(str, "%Y-%m-%d %T%.3f")
            .ok()
            .unwrap(),
    )
}

pub struct DateFormat {
    pub parser: fn(&str) -> DateTime<Utc>,
    pub regex: Regex,
    pub start_offset: usize,
    pub end_offset: usize,
}

pub fn guess_date_format(line: &str) -> Option<DateFormat> {
    let format = KNOWN_FORMATS
        .iter()
        .find(|f| Regex::new(f.0).unwrap().is_match(line));
    format.map(|f| {
        let regex = Regex::new(f.0).unwrap();
        let m = regex.find(line).unwrap();
        DateFormat {
            parser: f.1,
            regex,
            start_offset: m.start(),
            end_offset: m.end(),
        }
    })
}

#[cfg(test)]
fn test_should_guess(line: &str, expected: DateTime<Utc>) {
    let fmt = guess_date_format(line).unwrap();
    assert_eq!(true, fmt.regex.is_match(line));
    assert_eq!(
        expected,
        (fmt.parser)(&line[fmt.start_offset..fmt.end_offset])
    );
}

#[test]
fn should_guess_tomee() {
    test_should_guess("23-Apr-2020 00:00:00.001 INFO [EjbTimerPool - 57924] com.config.TestTimer.runTimer Starting config promotion...",
        Utc.ymd(2020, 4, 23).and_hms_milli(0, 0, 0, 1)
    );
}

#[test]
fn should_guess_journalctl() {
    test_should_guess("Apr 26 10:05:02 localhost.localdomain kernel: x86/fpu: Supporting XSAVE feature 0x010: 'MPX CSR'",
        Utc.ymd(2020, 4, 26).and_hms(10, 5, 2));
}

#[test]
fn should_guess_apache() {
    test_should_guess(
        "127.0.0.1 - frank [10/Oct/2000:13:55:36 -0700] \"GET /apache_pb.gif HTTP/1.0\" 200 2326",
        Utc.ymd(2000, 10, 10).and_hms(20, 55, 36),
    );
}

#[test]
fn should_guess_postgres() {
    // unfortunately chrono doesn't know how to parse timezone
    // names likes MST.
    test_should_guess(
        "2014-11-12 16:28:21.700 MST,,,2993,,5463ed15.bb1,1,,2014-11-12 16:28:21 MST,,0,LOG,00000,\"database system was shut down at 2014-11-12 16:28:16 MST\",,,,,,,,,\"\"\"",
        Utc.ymd(2014, 11, 12).and_hms_milli(16, 28, 21, 700),
    );
}

#[test]
fn should_guess_postgres2() {
    // unfortunately chrono doesn't know how to parse timezone
    // names likes MST.
    test_should_guess(
        "< 2020-04-21 03:31:00.486 CEST >STATEMENT:  insert into build_data_storage (build_id, metric_id, metric_value) values ($1,$2,$3)",
        Utc.ymd(2020, 4, 21).and_hms_milli(3, 31, 0, 486),
    );
}

// tests the format from the 'ts' util from moreutils
// https://unix.stackexchange.com/a/26797/36566
#[test]
fn should_parse_ts() {
    let line = "Apr 26 17:12:31 -rw-rw-r-- 1 emmanuel emmanuel 3.5K Apr 26 11:37 Cargo.lock";
    let fmt = guess_date_format(&line).unwrap();
    assert_eq!(true, fmt.regex.is_match(&line));
    assert_eq!(
        Utc.ymd(2020, 4, 26).and_hms(17, 12, 31),
        (fmt.parser)(&line[fmt.start_offset..fmt.end_offset])
    );
}
