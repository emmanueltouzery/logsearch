use chrono::prelude::*;
use regex::Regex;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
mod dateformat;

fn main() -> std::io::Result<()> {
    let mut args = env::args().skip(1);
    // https://stackoverflow.com/a/49964042/516188
    let is_piped_input = !atty::is(atty::Stream::Stdin);
    let mut reader: Box<dyn BufRead> = if is_piped_input {
        // stdin is not a tty, reading from it
        Box::new(BufReader::new(std::io::stdin()))
    } else {
        // stdin is a tty, the first param must be a filename
        match args.next() {
            None => display_help_and_exit(),
            Some(v) if v == "--help" => display_help_and_exit(),
            Some(v) if v == "--version" => display_version_and_exit(),
            Some(fname) => {
                let file = File::open(fname)?;
                Box::new(BufReader::new(file))
            }
        }
    };

    // if the input is piped (eg the data may be coming in realtime)
    // and the output is not (we are displaying to a terminal), we will
    // display realtime progress info to the user, that we'll replace
    // with final data when we get it.
    let is_piped_output = !atty::is(atty::Stream::Stdout);
    let is_display_preview = is_piped_input && !is_piped_output;

    let mut patterns: Vec<String> = args.collect();
    if patterns.contains(&"--version".to_string()) {
        display_version_and_exit();
    }
    if patterns.is_empty() {
        display_help_and_exit();
    }
    let dtfmt = if let Some(pos) = patterns.iter().position(|p| p == "--dtfmt") {
        patterns.remove(pos);
        Some(patterns.remove(pos))
    } else {
        None
    };

    let pattern_regexes: Vec<_> = patterns
        .iter()
        .map(|p| Regex::new(&p).expect("Invalid pattern regex"))
        .collect();

    process_input(
        &mut reader,
        is_display_preview,
        &patterns,
        &pattern_regexes,
        dtfmt.as_ref().map(|s| dateformat::build_custom_format(s)),
    )
}

struct InRangeState {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    pattern_idx: usize,
    match_count: usize,
}

enum ParsingState {
    NotInRange,
    InRange(InRangeState),
}

fn process_input(
    reader: &mut dyn BufRead,
    is_display_preview: bool,
    patterns: &[String],
    pattern_regexes: &[Regex],
    dtfmt: Option<dateformat::DateFormat>,
) -> std::io::Result<()> {
    let mut buffer = String::new();
    let mut cur_timestamp = None::<DateTime<Utc>>;
    let mut state = ParsingState::NotInRange;
    let datefmt = match dtfmt {
        Some(f) => f,
        None => match guess_dateformat(reader)? {
            Some(f) => f,
            None => {
                eprintln!("Gave up guessing the date format. Consider giving the date format through the --dtfmt parameter");
                std::process::exit(1);
            }
        },
    };

    while reader.read_line(&mut buffer)? != 0 {
        // extract the timestamp from the line if present
        if let Some(timestamp_str) = datefmt.regex.find(&buffer).map(|m| m.as_str()) {
            let ts = (datefmt.parser)(&datefmt.fmt, timestamp_str);
            cur_timestamp = Some(ts);
            match state {
                ParsingState::InRange(ref st) if ts - st.end > chrono::Duration::minutes(5) => {
                    // too long interval, close the current range
                    print_pattern(is_display_preview, &st, &patterns);
                    state = ParsingState::NotInRange;
                }
                _ => {}
            }
        }

        // if we have a timestamp, search for one of the patterns in the line
        if let Some(cur_timestamp) = cur_timestamp {
            match (
                pattern_regexes.iter().position(|p| p.is_match(&buffer)),
                &mut state,
            ) {
                (Some(idx), ParsingState::InRange(ref mut st)) if idx == st.pattern_idx => {
                    // prolonging the current pattern
                    st.end = cur_timestamp;
                    st.match_count += 1;
                }
                (Some(idx), ParsingState::InRange(st)) => {
                    // hit a different pattern
                    print_pattern(is_display_preview, &st, &patterns);
                    state = start_range(is_display_preview, idx, cur_timestamp, &patterns)?;
                }
                (Some(idx), ParsingState::NotInRange) => {
                    state = start_range(is_display_preview, idx, cur_timestamp, &patterns)?;
                }
                (None, _) => {}
            }
        }
        buffer.clear();
    }
    if let ParsingState::InRange(st) = state {
        // print the final contents of the pattern when the input finishes
        print_pattern(is_display_preview, &st, &patterns);
    }
    Ok(())
}

fn display_help_and_exit() -> ! {
    eprintln!(
        "parameters: <log filename> [--dtfmt dateformat] <pattern> [extra patterns]
if data is passed by the standard input (piped in) then no need to pass log filename
documentation for the dateformat: https://docs.rs/chrono/0.4.11/chrono/format/strftime."
    );
    std::process::exit(1);
}

fn display_version_and_exit() -> ! {
    println!("version {}", env!("CARGO_PKG_VERSION"));
    std::process::exit(1);
}

fn start_range(
    is_display_preview: bool,
    idx: usize,
    cur_timestamp: DateTime<Utc>,
    patterns: &[String],
) -> std::io::Result<ParsingState> {
    if is_display_preview {
        // this info may be overwritten later if the input is piped
        // hence we don't send a \n
        print!(
            "{} -> ? [{}] -- ongoing",
            cur_timestamp.format("%Y-%m-%d %T"),
            patterns[idx]
        );
        std::io::stdout().flush()?;
    }
    Ok(ParsingState::InRange(InRangeState {
        start: cur_timestamp,
        end: cur_timestamp,
        pattern_idx: idx,
        match_count: 1,
    }))
}

fn guess_dateformat(reader: &mut dyn BufRead) -> std::io::Result<Option<dateformat::DateFormat>> {
    let mut buffer = String::new();
    let mut datefmt_attempts = 0;
    while reader.read_line(&mut buffer)? != 0 {
        let datefmt = dateformat::guess_date_format(&buffer);
        if datefmt.is_none() {
            datefmt_attempts += 1;
            if datefmt_attempts >= 5 {
                break;
            }
        } else {
            return Ok(datefmt);
        }
        buffer.clear();
    }
    Ok(None)
}

fn print_pattern(is_display_preview: bool, state: &InRangeState, patterns: &[String]) {
    if is_display_preview {
        // \r to clear the stdout if we had a piped output & progress report
        print!("\r");
    }
    println!(
        "{} -> {}: [{}] {} matches",
        state.start.format("%Y-%m-%d %T"),
        state.end.format("%Y-%m-%d %T"),
        patterns[state.pattern_idx],
        state.match_count
    );
}
