use chrono::prelude::*;
use regex::Regex;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;

struct InRangeState {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    cur_pattern: usize,
    match_count: usize,
}

enum ParsingState {
    NotInRange,
    InRange(InRangeState),
}

fn main() -> std::io::Result<()> {
    let mut args = env::args().skip(1);
    let fname = args.next();
    let patterns: Vec<String> = args.collect();
    if patterns.is_empty() {
        eprintln!("parameters: <log filename> <pattern> [extra patterns]");
        std::process::exit(1);
    }
    let fname = fname.unwrap();

    let datetime_regex = Regex::new(r"^\d\d-\w{3}-\d{4} \d\d:\d\d:\d\d\.\d{3}").unwrap();
    let date_fmt = "%d-%b-%Y %T.%f";

    let file = File::open(fname)?;
    let reader = BufReader::new(file);

    let pattern_regexes: Vec<_> = patterns
        .iter()
        .map(|p| Regex::new(&p).expect("Invalid pattern regex"))
        .collect();

    let mut cur_timestamp = None::<DateTime<Utc>>;
    let mut state = ParsingState::NotInRange;
    for line in reader.lines() {
        let line = line?;

        // extract the timestamp from the line if present
        if let Some(timestamp_str) = datetime_regex.find(&line).map(|m| m.as_str()) {
            let ts = Utc.from_utc_datetime(
                &NaiveDateTime::parse_from_str(timestamp_str, date_fmt)
                    .ok()
                    .unwrap(),
            );
            cur_timestamp = Some(ts);
            match state {
                ParsingState::InRange(ref st) if ts - st.end > chrono::Duration::minutes(5) => {
                    // too long interval, close the current range
                    print_pattern(&st, &patterns);
                    state = ParsingState::NotInRange;
                }
                _ => {}
            }
        }

        // if we have a timestamp, search for one of the patterns in the line
        if let Some(cur_timestamp) = cur_timestamp {
            match (
                pattern_regexes.iter().position(|p| p.is_match(&line)),
                &mut state,
            ) {
                (Some(idx), ParsingState::InRange(ref mut st)) if idx == st.cur_pattern => {
                    // prolonging the current pattern
                    st.end = cur_timestamp;
                    st.match_count += 1;
                }
                (Some(idx), ParsingState::InRange(st)) => {
                    // hit a different pattern
                    print_pattern(&st, &patterns);
                    state = ParsingState::InRange(InRangeState {
                        start: cur_timestamp,
                        end: cur_timestamp,
                        cur_pattern: idx,
                        match_count: 1,
                    });
                }
                (Some(idx), ParsingState::NotInRange) => {
                    state = ParsingState::InRange(InRangeState {
                        start: cur_timestamp,
                        end: cur_timestamp,
                        cur_pattern: idx,
                        match_count: 1,
                    });
                }
                (None, _) => {}
            }
        }
    }
    Ok(())
}

fn print_pattern(state: &InRangeState, patterns: &[String]) {
    println!(
        "{} -> {}: [{}] {} matches",
        state.start.format("%Y-%m-%d %T"),
        state.end.format("%Y-%m-%d %T"),
        patterns[state.cur_pattern],
        state.match_count
    );
}
