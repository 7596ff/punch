extern crate clap;
extern crate chrono;

use std::env;
use std::fs::DirBuilder;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Write;
use std::io::Read;
use std::io;
use std::io::Seek;
use std::io::SeekFrom;
use std::ops::Sub;
use std::path::PathBuf;
use std::process;
use std::str;

use clap::{App, AppSettings, SubCommand};

use chrono::DateTime;
use chrono::TimeZone;
use chrono::UTC;


const RECORD_LENGTH: usize = 22;

fn main() {
    match ensure_log_file_exists() {
    	Ok(_) => {},
    	Err(e) => {
    		println!("Couldn't create punch log: {}.\nExiting.", e);
			process::exit(1)
    	}
    }

    let args = App::new("punchcard")
        .about("Time tracker")
        .setting(AppSettings::ArgRequiredElseHelp)
        .subcommand(SubCommand::with_name("in"))
        .subcommand(SubCommand::with_name("out"))
        .subcommand(SubCommand::with_name("card"))
        .get_matches();

	if args.is_present("in") {
		ensure_last_record_is_of_action(Action::PunchOut);
		write_record_to_log(chrono::UTC::now(), Action::PunchIn);
	}
	
	if args.is_present("out") {
		ensure_last_record_is_of_action(Action::PunchIn);
		write_record_to_log(chrono::UTC::now(), Action::PunchOut);
	}
	
	if args.is_present("card") {
		print_last_record();
	}
}

#[derive(Debug)]
#[derive(PartialEq)]
enum Action {
	PunchIn,
	PunchOut,
	Unset
}

struct Record {
	timestamp: DateTime<UTC>,
	action: Action
}

fn ensure_last_record_is_of_action(expected_action: Action) {

	let mut config_file = get_conf_file(true, false).unwrap();
    let mut record = empty_record();

    match populate_record_at_offset_from_end(&mut config_file, &mut record, 0) {
    	Ok(_) => {},
    	Err(e) => {
    		println!("Couldn't create punch log: {}.\nExiting.", e);
			process::exit(1)
    	}
    }
    
    if record.action != expected_action {
    	match expected_action {
    		Action::PunchIn => {
    			println!("Already punched out, punch in first!");
				process::exit(1)    			
    		}
    		Action::PunchOut => {
    			println!("Already punched in, punch out first!");
    			process::exit(1)
    		}
    		Action::Unset => {
    			println!("Found unset action, log file corrupt");
    			process::exit(1)
    		}
    	}
    	
    }
}

fn get_conf_file(read: bool, append: bool) -> io::Result<File> {
	
	let mut conf_file = PathBuf::new();
    conf_file.push(env::home_dir().unwrap());
    conf_file.push(".punch");
    conf_file.push("punch.log");
    
    OpenOptions::new().read(read).append(append).open(conf_file)
}

fn empty_record() -> Record {
	Record {
    	action: Action::Unset,
    	timestamp: chrono::UTC::now()
    }
}

fn print_last_record() {
    let mut config_file = get_conf_file(true, false).unwrap();
    let mut record = empty_record();

    match populate_record_at_offset_from_end(&mut config_file, &mut record, 0) {
    	Ok(_) => {},
    	Err(e) => {
    		println!("Couldn't create punch log: {}.\nExiting.", e);
			process::exit(1)
    	}
    }
    
    if record.action == Action::PunchIn {
    	let current_timestamp = chrono::UTC::now();
    	let time_punched_in = current_timestamp.sub(record.timestamp);
    	println!("Punched in since {} ({})", record.timestamp, time_punched_in)
    } 
    else {
    	let mut previous_record = empty_record();
    	match populate_record_at_offset_from_end(&mut config_file, &mut previous_record, 1) {
	    	Ok(_) => {},
	    	Err(e) => {
	    		println!("Couldn't create punch log: {}.\nExiting.", e);
				process::exit(1)
	    	}
	    }
    	
    	let delta = record.timestamp.sub(previous_record.timestamp);
    	println!("Previously punched in between {} and {} ({})", 
    		previous_record.timestamp, record.timestamp, delta)
    }
}

fn populate_record_at_offset_from_end(config_file: &mut File, record: &mut Record, offset_from_end: u64) -> Result<(), String> {
	seek_to_record_offset(config_file, offset_from_end);
	populate_record_at_current_offset(config_file, record);
	Ok(())
}


fn write_record_to_log(tm: DateTime<UTC>, action: Action) {
	let action_token = match action {
		Action::PunchIn => "I",
		Action::PunchOut => "O",
		Action::Unset => "U"
	};
	
    let mut config_file = get_conf_file(false, true).unwrap();
    let fmt = tm.format("%FT%T");
	let formatted_timestamp = fmt.to_string();
	do_file_write(formatted_timestamp.as_bytes(), &mut config_file);
	do_file_write("_".as_bytes(), &mut config_file);
	do_file_write(action_token.as_bytes(), &mut config_file);
	do_file_write("\n".as_bytes(), &mut config_file);
}

fn populate_record_at_current_offset(f: &mut File, record: &mut Record) -> bool {
	let mut data = [0 as u8; RECORD_LENGTH];
	let read = f.read(&mut data);
	if read.unwrap() != RECORD_LENGTH {
		panic!("Could not read complete record")
	}
	let (ts_data, rest) = data.split_at(19);
	let timestamp = str::from_utf8(&ts_data).unwrap();
	let parse_result = chrono::UTC.datetime_from_str(&timestamp, "%FT%T");
	
	let record_ts = parse_result.
		unwrap().with_timezone(&chrono::UTC);
	record.timestamp = record_ts;
	let action_string = str::from_utf8(&rest).unwrap();
	if action_string == "_O\n" {
		record.action = Action::PunchOut;
	}
	else if action_string == "_I\n" {
		record.action = Action::PunchIn;
	} 
	else {
		panic!("Could not determine action type from '{}'", action_string)
	}
	return true
}

fn seek_to_record_offset(f: &mut File, record_offset: u64) {
	let m = f.metadata().unwrap();
	let file_len = m.len();
	
	let record_length_in_bytes = RECORD_LENGTH as u64;
	let seek_offset = file_len - ((record_offset + 1) * record_length_in_bytes);
	
	if f.seek(SeekFrom::Start(seek_offset)).unwrap() != seek_offset {
		panic!("Could not seek to record offset {}", record_offset)
	}
}

fn do_file_write(data: &[u8], f: &mut File) {
	match f.write_all(data) {
    	Ok(_) => {},
    	Err(e) => println!("Failed to write data to log: {}", e)
    }
}

fn ensure_log_file_exists() -> io::Result<()> {
    let mut conf_dir = PathBuf::new();
    conf_dir.push(env::home_dir().unwrap());
    conf_dir.push(".punch");
    let config_path = conf_dir.as_path();

    let mut conf_file_builder = PathBuf::from(config_path);
    conf_file_builder.push("punch.log");

    let mut dir_builder = DirBuilder::new();
    dir_builder.recursive(true);
    
    try!(dir_builder.create(config_path));

    let conf_file = conf_file_builder.as_path();
    match OpenOptions::new().create(true).write(true).open(conf_file) {
    	Ok(_) => Ok(()),
    	Err(e) => Err(e)
    }	
}