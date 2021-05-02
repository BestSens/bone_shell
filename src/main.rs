use std::io::*;
use structopt::StructOpt;
use bone_api::Bone;
use atty::Stream;
use std::time::Instant;

use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
pub struct Opt {
	#[structopt(short, long, default_value = "localhost")]
	connect: String,

	#[structopt(short, long, default_value = "6450")]
	port: String,

	#[structopt(short, long)]
	msgpack: bool,

	#[structopt(short, long)]
	no_pretty: bool,

	#[structopt(short, long)]
	response_time: bool,

	#[structopt(long)]
	username: Option<String>,

	#[structopt(long)]
	password: Option<String>,

	#[structopt(long, default_value = "2")]
	api: u32,

	command: Option<String>
}

fn main() -> std::io::Result<()> {
	let opt = Opt::from_args();

	let ip = opt.connect;
	let port = opt.port;

	let mut bone1 = Bone::new(&ip, &port, opt.msgpack);
	bone1.connect();
	
	if let Some(username) = &opt.username {
		if let Some(password) = &opt.password {
			let result = bone1.login(username, password);
			match result {
				Err(msg) => panic!("Error while logging in: {}", msg),
				_ => ()
			}
		} else {
			panic!("--username supplied without --password");
		}
	}

	let data = match bone1.send_command(&json::object!{"command" => "serial_number"}) {
		Ok(n) => n,
		Err(_err) => json::object!{"error" => "missing"},
	};

	if let Some(command) = &opt.command {
		// command mode
		let start = Instant::now();
		let mut command = json::parse(&command).unwrap();
		command["api"] = opt.api.into();
		let parsed = bone1.send_command(&command).unwrap();
		let duration = start.elapsed().as_millis();
		
		let pretty_response;

		if opt.no_pretty {
			pretty_response = json::stringify(parsed);
		} else {
			pretty_response = json::stringify_pretty(parsed, 4);
		}

		if opt.response_time {
			writeln_dimmed(&format!("took {} ms", duration))?;
		}

		println!("{}", pretty_response);
	} else if !atty::is(Stream::Stdin) {
		// pipe mode
		let mut command = String::new();
		stdin().read_line(&mut command).unwrap();

		let start = Instant::now();
		let mut command = json::parse(&command).unwrap();
		command["api"] = opt.api.into();
		let parsed = bone1.send_command(&command).unwrap();
		let duration = start.elapsed().as_millis();

		let pretty_response;

		if opt.no_pretty {
			pretty_response = json::stringify(parsed);
		} else {
			pretty_response = json::stringify_pretty(parsed, 4);
		}

		if opt.response_time {
			writeln_dimmed(&format!("took {} ms", duration))?;
		}

		println!("{}", pretty_response);
	} else {
		// shell mode
		let alias = &data["payload"]["alias"];
		let cnt_str;

		if !alias.is_string() {
			let serial_number = &data["payload"]["serial_number"];

			if !serial_number.is_string() {
				cnt_str = String::from("");
			} else {
				cnt_str = serial_number.to_string();
			}
		} else {
			cnt_str = alias.to_string();
		}

		loop {
			print!("{} > ", cnt_str);
			stdout().flush().unwrap();

			let mut command = String::new();
			stdin().read_line(&mut command).unwrap();

			if let Some(first_char) = command.chars().next() {
				if first_char != '{' && first_char != '[' {
					let tmp_len = command.trim_end().len();
					command.truncate(tmp_len);

					if command == "q" || command == "quit" || command == "exit" {
						return Ok(())
					}

					let chunks: Vec<&str> = command.split_whitespace().collect();

					if chunks.len() == 1 {
						command = json::object!{"command": chunks[0].clone()}.dump();
					} else if chunks.len() == 2 {
						let payload = match json::parse(&chunks[1]) {
							Ok(n) => n,
							Err(err) => { eprintln!("error parsing payload: {}", err); continue; },
						};
						command = json::object!{"command": chunks[0].clone(), "payload": payload}.dump();
					} else {
						continue;
					}
				}
			} else {
				continue;
			}

			let result = json::parse(&command);
			match result {
				Err(msg) => eprintln!("invalid input: {}", msg),
				Ok(mut command) => {
					command["api"] = opt.api.into();
					let start = Instant::now();
					let parsed = match bone1.send_command(&command) {
						Ok(n) => n,
						Err(err) => { eprintln!("Error: {}", err); continue; },
					};
					let duration = start.elapsed().as_millis();

					let pretty_response;

					if opt.no_pretty {
						pretty_response = json::stringify(parsed);
					} else {
						pretty_response = json::stringify_pretty(parsed, 4);
					}
					
					writeln_dimmed(&command.dump())?;

					if opt.response_time {
						writeln_dimmed(&format!("took {} ms", duration))?;
					}

					println!("{}", pretty_response);
				}
			}
		}
	}

	Ok(())
}

fn writeln_dimmed(output: &str) -> Result<()> {
	let mut stdout = StandardStream::stdout(ColorChoice::Always);
	stdout.set_color(ColorSpec::new().set_fg(Some(Color::Rgb(150, 150, 150))).set_italic(true))?;
	match writeln!(&mut stdout, "# {}", output) {
		Ok(()) => (),
		Err(_err) => stdout.set_color(&ColorSpec::new())?,
	};
	stdout.set_color(&ColorSpec::new())?;
	Ok(())
}