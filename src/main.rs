#![deny(clippy::all, clippy::pedantic, unused, warnings)]

mod journal;

use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::ops::Add;
use std::ops::Sub;
use std::process;
use std::str;

use clap::{App, AppSettings, Arg, SubCommand};

use chrono::DateTime;
use chrono::Datelike;
use chrono::TimeZone;
use chrono::Timelike;
use chrono::UTC;

const RECORD_LENGTH: usize = 22;

#[derive(Debug, PartialEq)]
enum Action {
    PunchIn,
    PunchOut,
    Unset,
}

#[derive(Debug)]
struct Record {
    timestamp: DateTime<UTC>,
    action: Action,
}

#[derive(Debug)]
struct DailyDuration {
    date: chrono::date::Date<UTC>,
    duration: chrono::Duration,
}

fn main() {
    journal::exit_if_log_file_cannot_be_created();

    let args = App::new("Punch")
        .about("A simple time tracker app")
        .version("0.1")
        .setting(AppSettings::ArgRequiredElseHelp)
        .subcommand(SubCommand::with_name("in").about("Punch in"))
        .subcommand(SubCommand::with_name("out").about("Punch out"))
        .subcommand(
            SubCommand::with_name("card")
                .about("Display state")
                .arg(
                    Arg::with_name("week")
                        .long("week")
                        .short("w")
                        .help("Display summary for the last week"),
                )
                .arg(
                    Arg::with_name("mtd")
                        .long("mtd")
                        .short("m")
                        .help("Display summary for the month to date"),
                ),
        )
        .get_matches();

    match args.subcommand() {
        ("card", Some(specifier)) => {
            if specifier.is_present("week") {
                print_weekly_summary();
            } else if specifier.is_present("mtd") {
                print_month_to_date_summary();
            } else {
                print_current_state();
            }
        }
        ("in", _) => {
            ensure_last_record_is_of_action(&Action::PunchOut);
            write_record_to_log(chrono::UTC::now(), &Action::PunchIn);
        }
        ("out", _) => {
            ensure_last_record_is_of_action(&Action::PunchIn);
            write_record_to_log(chrono::UTC::now(), &Action::PunchOut);
        }
        _ => {
            println!("Unknown command");
        }
    }
}

fn write_record_to_log(tm: DateTime<UTC>, action: &Action) {
    let action_token = match action {
        Action::PunchIn => "I",
        Action::PunchOut => "O",
        Action::Unset => "U",
    };

    let mut config_file = journal::get_conf_file(false, true).unwrap();
    let fmt = tm.format("%FT%T");
    let formatted_timestamp = fmt.to_string();
    journal::append_to_file(
        format!("{}_{}\n", formatted_timestamp, action_token).as_bytes(),
        &mut config_file,
    );
}

fn print_month_to_date_summary() {
    let mut start_of_month = chrono::UTC::now()
        .with_second(0)
        .map(|ts| ts.with_minute(0).map(|ts| ts.with_hour(0)))
        .unwrap()
        .unwrap()
        .unwrap();

    loop {
        if start_of_month.day() == 1 {
            break;
        }
        start_of_month = start_of_month.sub(chrono::Duration::days(1));
    }

    print_daily_durations_since(start_of_month);
}

fn print_weekly_summary() {
    let mut start_of_week = chrono::UTC::now()
        .with_second(0)
        .map(|ts| ts.with_minute(0).map(|ts| ts.with_hour(0)))
        .unwrap()
        .unwrap()
        .unwrap();

    loop {
        if start_of_week.weekday() == chrono::Weekday::Mon {
            break;
        }
        start_of_week = start_of_week.sub(chrono::Duration::days(1));
    }

    print_daily_durations_since(start_of_week);
}

fn print_daily_durations_since(start_time: chrono::DateTime<UTC>) {
    let mut daily_durations: Vec<DailyDuration> = vec![];
    let mut record_offset = 0;
    let mut record = empty_record();
    let mut config_file = journal::get_conf_file(true, false).unwrap();
    let mut current_date: chrono::Date<UTC> =
        chrono::UTC::now().date().add(chrono::Duration::days(1));

    let mut day_count: i64 = 0;
    let mut total_seconds_in_current_day: i64 = 0;
    let mut total_seconds_in_time_range: i64 = 0;

    // TODO need to account for duration between now and last punch-in
    if get_last_record_action() == Action::PunchIn {
        record_offset = 1;
    }

    let mut last_punch_out_timestamp: chrono::DateTime<UTC> = chrono::UTC::now();

    loop {
        let read_attempt =
            populate_record_at_offset_from_end(&mut config_file, &mut record, record_offset);
        if read_attempt.is_err() || record.timestamp < start_time {
            if total_seconds_in_current_day != 0 {
                daily_durations.push(DailyDuration {
                    date: current_date,
                    duration: chrono::Duration::seconds(total_seconds_in_current_day),
                });
            }
            break;
        }
        if record.timestamp.date() != current_date && day_count != 0 {
            daily_durations.push(DailyDuration {
                date: current_date,
                duration: chrono::Duration::seconds(total_seconds_in_current_day),
            });

            total_seconds_in_current_day = 0;
        }

        if record.action == Action::PunchOut {
            last_punch_out_timestamp = record.timestamp;
        } else {
            total_seconds_in_current_day +=
                last_punch_out_timestamp.sub(record.timestamp).num_seconds();
            total_seconds_in_time_range +=
                last_punch_out_timestamp.sub(record.timestamp).num_seconds();
        }

        record_offset += 1;
        current_date = record.timestamp.date();
        day_count += 1;
    }

    daily_durations.reverse();

    for daily_duration in &daily_durations {
        println!(
            "{}: {}",
            daily_duration.date,
            format_duration(daily_duration.duration)
        );
    }

    println!(
        "\nTotal: {}",
        format_duration(chrono::Duration::seconds(total_seconds_in_time_range))
    );
}

fn print_current_state() {
    let mut config_file = journal::get_conf_file(true, false).unwrap();
    let mut record = empty_record();

    match populate_record_at_offset_from_end(&mut config_file, &mut record, 0) {
        Ok(_) => {}
        Err(e) => {
            println!("Couldn't read entry: {}.\nExiting.", e);
            process::exit(1)
        }
    }

    if record.action == Action::PunchIn {
        let current_timestamp = chrono::UTC::now();
        let time_punched_in = current_timestamp.sub(record.timestamp);

        println!(
            "Punched in since {} ({})",
            record.timestamp,
            format_duration(time_punched_in)
        );
    } else {
        let mut previous_record = empty_record();
        match populate_record_at_offset_from_end(&mut config_file, &mut previous_record, 1) {
            Ok(_) => {}
            Err(e) => {
                println!("Couldn't read entry: {}.\nExiting.", e);
                process::exit(1)
            }
        }

        let delta = record.timestamp.sub(previous_record.timestamp);

        println!(
            "Previously punched in between {} and {} ({})",
            previous_record.timestamp,
            record.timestamp,
            format_duration(delta)
        );
    }
}

fn format_duration(duration: chrono::Duration) -> String {
    format!(
        "{:02}h{:02}m",
        duration.num_hours(),
        duration.num_minutes() % 60
    )
}

fn get_last_record_action() -> Action {
    let mut config_file = journal::get_conf_file(true, false).unwrap();
    let mut record = empty_record();

    if config_file.metadata().unwrap().len() == 0 {
        return Action::Unset;
    }

    match populate_record_at_offset_from_end(&mut config_file, &mut record, 0) {
        Ok(_) => {}
        Err(e) => {
            println!("Couldn't create punch log: {}.\nExiting.", e);
            process::exit(1)
        }
    }

    record.action
}

fn ensure_last_record_is_of_action(expected_action: &Action) {
    let last_action = get_last_record_action();

    if last_action == Action::Unset {
        return;
    }

    if last_action != *expected_action {
        match expected_action {
            Action::PunchIn => {
                println!("Already punched out, punch in first!");
                process::exit(0)
            }
            Action::PunchOut => {
                println!("Already punched in, punch out first!");
                process::exit(0)
            }
            Action::Unset => {
                // log file could be empty, this is ok.
            }
        }
    }
}

fn empty_record() -> Record {
    Record {
        action: Action::Unset,
        timestamp: chrono::UTC::now(),
    }
}

fn populate_record_at_offset_from_end(
    config_file: &mut File,
    record: &mut Record,
    offset_from_end: u64,
) -> Result<(), String> {
    seek_to_record_offset(config_file, offset_from_end)
        .and_then(|_| populate_record_at_current_offset(config_file, record))
}

fn populate_record_at_current_offset(f: &mut File, record: &mut Record) -> Result<(), String> {
    let mut data = [0_u8; RECORD_LENGTH];
    let read = f.read(&mut data);
    if read.unwrap() != RECORD_LENGTH {
        panic!("Could not read complete record of {} bytes", RECORD_LENGTH);
    }
    let (ts_data, rest) = data.split_at(19);
    let timestamp = str::from_utf8(ts_data).unwrap();
    let parse_result = chrono::UTC.datetime_from_str(timestamp, "%FT%T");

    let record_ts = parse_result.unwrap().with_timezone(&chrono::UTC);
    record.timestamp = record_ts;
    let action_string = str::from_utf8(rest).unwrap();
    if action_string == "_O\n" {
        record.action = Action::PunchOut;
    } else if action_string == "_I\n" {
        record.action = Action::PunchIn;
    } else {
        return Err(format!(
            "Could not determine action type from '{}'",
            action_string
        ));
    }
    Ok(())
}

fn seek_to_record_offset(f: &mut File, record_offset: u64) -> Result<(), String> {
    let m = f.metadata().unwrap();
    let file_len = m.len();

    if file_len < RECORD_LENGTH as u64 {
        return Err(String::from("No data in log - punch in first!"));
    }

    let record_length_in_bytes = RECORD_LENGTH as u64;
    let seek_offset = file_len - ((record_offset + 1) * record_length_in_bytes);
    let seek_result = f.seek(SeekFrom::Start(seek_offset));
    if seek_result.is_err() {
        return Err(format!("Failed to seek: {}", seek_result.err().unwrap()));
    }
    if seek_result.unwrap() != seek_offset {
        return Err(format!("Could not seek to record offset {}", seek_offset));
    }
    Ok(())
}
