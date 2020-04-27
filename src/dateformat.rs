// I tried https://docs.rs/crate/dtparse/1.0.3 but it didn't work well enough
// for me, plus it doesn't return a format string
use chrono::prelude::*;
use regex::Regex;

type DateTimeParser = fn(&str, &str) -> DateTime<Utc>;

// i return the format string+the function to call it with.
// i also had a version with a boxed dyn closure that would
// take only one parameter, no need for the fmt, but this
// in theory should be faster (although i couldn't measure it)
pub struct DateFormat {
    pub fmt: String,
    pub parser: DateTimeParser,
    pub regex: Regex,
}

const KNOWN_FORMATS: &[&str] = &[
    // "23-Apr-2020 00:00:00.001" -- tomee log format
    "%d-%b-%Y %T%.3f",
    // "Apr 26 10:05:02" -- journalctl
    "%b %d %T",
    // 10/Oct/2000:13:55:36 -0700 -- apache
    "%d/%b/%Y:%T %z",
    // 2014-11-12 16:28:21.700 MST -- postgres
    "%Y-%m-%d %T%.3f",
];

pub fn guess_date_format(line: &str) -> Option<DateFormat> {
    let format = KNOWN_FORMATS
        .iter()
        .find(|f| regex_for_format_str(f).is_match(line));
    format.map(|f| build_custom_format(*f))
}

fn regex_for_format_str(fmt: &str) -> Regex {
    Regex::new(
        &fmt.replace("%Y", r"\d{4}")
            .replace("%C", r"\d\d")
            .replace("%y", r"\d\d")
            .replace("%m", r"[0-1]\d")
            .replace("%B", r"\w")
            .replace("%A", r"\w")
            .replace("%h", r"\w{3}")
            .replace("%a", r"\w{3}")
            .replace("%d", r"[0-3]\d")
            .replace("%e", r"[ 1]\d")
            .replace("%w", r"[0-6]")
            .replace("%u", r"[1-7]")
            .replace("%U", r"\d\d")
            .replace("%W", r"\d\d")
            .replace("%G", r"\d{4}")
            .replace("%g", r"\d\d")
            .replace("%V", r"\d\d")
            .replace("%j", r"\d{3}")
            .replace("%D", r"\d\d/\d\d/\d\d")
            .replace("%x", r"\d\d/\d\d/\d\d")
            .replace("%F", r"\d{4}-\d\d-\d\d")
            .replace("%v", r"[ 1]\d-\w{3}-\d{4}")
            .replace("%H", r"\d\d")
            .replace("%k", r"[ 1]\d")
            .replace("%I", r"[01]\d")
            .replace("%l", r"[ 1]\d")
            .replace("%P", r"(am|pm)")
            .replace("%p", r"(AM|PM)")
            .replace("%M", r"\d\d")
            .replace("%S", r"\d\d")
            .replace("%f", r"\d+")
            .replace("%.f", r"\.\d+")
            .replace("%.3f", r"\.\d{3}")
            .replace("%.6f", r"\.\d{6}")
            .replace("%.9f", r"\.\d{9}")
            .replace("%3f", r"\d{3}")
            .replace("%6f", r"\d{6}")
            .replace("%9f", r"\d{9}")
            .replace("%R", r"\d\d:\d\d")
            .replace("%T", r"\d\d:\d\d:\d\d")
            .replace("%X", r"\d\d:\d\d:\d\d")
            .replace("%r", r"\d\d:\d\d:\d\d (AM|PM)")
            .replace("%z", r"[+-]\d{4}")
            .replace("%:z", r"[+-]\d\d:\d\d")
            .replace("%#z", r"[+-]\d\d{2,4}")
            .replace("%s", r"\d+")
            .replace("%t", "\t")
            .replace("%n", "\n")
            .replace("%%", "%")
            .replace("%b", r"\w{3}"),
        // skipped %c ctime and %+ iso 8601+rfc3339. they'd better fit as autodetect i think.
    )
    .unwrap_or_else(|_| panic!("Invalid format string: {}", fmt))
}

pub fn build_custom_format(dtfmt: &str) -> DateFormat {
    let regex = regex_for_format_str(dtfmt);
    let fmt = dtfmt.to_string();
    match dtfmt {
        _ if ["%z", "%:z", "%#z"].iter().any(|p| dtfmt.contains(p)) => {
            // timezone info is present
            DateFormat { fmt, parser, regex }
        }
        _ if ["%Y", "%C", "%y", "%G", "%g", "%D", "%x", "%F", "%v", "%s"]
            .iter()
            .any(|p| dtfmt.contains(p)) =>
        {
            // no timezone
            DateFormat {
                fmt,
                parser: parser_no_tz,
                regex,
            }
        }
        _ => {
            // no TZ nor year
            DateFormat {
                fmt: format!("%Y-{}", fmt),
                parser: parser_no_tz_no_year,
                regex,
            }
        }
    }
}

fn parser(fmt: &str, str: &str) -> DateTime<Utc> {
    DateTime::parse_from_str(str, fmt).ok().unwrap().into()
}

fn parser_no_tz(fmt: &str, str: &str) -> DateTime<Utc> {
    Utc.from_utc_datetime(&NaiveDateTime::parse_from_str(str, fmt).ok().unwrap())
}

fn parser_no_tz_no_year(fmt: &str, str: &str) -> DateTime<Utc> {
    Utc.from_utc_datetime(
        &NaiveDateTime::parse_from_str(&format!("{}-{}", Local::now().year(), str), fmt)
            .ok()
            .unwrap(),
    )
}

#[cfg(test)]
fn test_should_guess(line: &str, expected: DateTime<Utc>) {
    let fmt = guess_date_format(line).unwrap();
    test_should_parse(&fmt, line, expected);
}

#[cfg(test)]
fn test_should_parse(fmt: &DateFormat, line: &str, expected: DateTime<Utc>) {
    assert_eq!(true, fmt.regex.is_match(line));
    assert_eq!(
        expected,
        (fmt.parser)(&fmt.fmt, fmt.regex.find(&line).map(|m| m.as_str()).unwrap())
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
    test_should_guess(
        "Apr 26 17:12:31 -rw-rw-r-- 1 emmanuel emmanuel 3.5K Apr 26 11:37 Cargo.lock",
        Utc.ymd(2020, 4, 26).and_hms(17, 12, 31),
    );
}

#[test]
fn build_custom_fmt() {
    let fmt = build_custom_format("%Y-%m-%d %T %z");
    test_should_parse(
        &fmt,
        "2019-12-26 17:12:31 +0200 -rw-rw-r-- 1 emmanuel emmanuel 3.5K Apr 26 11:37 Cargo.lock",
        Utc.ymd(2019, 12, 26).and_hms(15, 12, 31),
    );
}

#[test]
fn build_custom_fmt_notz() {
    let fmt = build_custom_format("%Y-%m-%d %T");
    test_should_parse(
        &fmt,
        "2019-12-26 17:12:31 -rw-rw-r-- 1 emmanuel emmanuel 3.5K Apr 26 11:37 Cargo.lock",
        Utc.ymd(2019, 12, 26).and_hms(17, 12, 31),
    );
}

#[test]
fn build_custom_fmt_notz_noyear() {
    let fmt = build_custom_format("%m-%d %T");
    test_should_parse(
        &fmt,
        "12-26 17:12:31 -rw-rw-r-- 1 emmanuel emmanuel 3.5K Apr 26 11:37 Cargo.lock",
        Utc.ymd(Local::now().year(), 12, 26).and_hms(17, 12, 31),
    );
}
