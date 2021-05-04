use std::io::*;
use structopt::StructOpt;
use bone_api::Bone;
use atty::Stream;
use std::time::Instant;

extern crate statistical;
extern crate rpassword;

use textplots::{Chart, Plot, Shape};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use terminal_size::{Width, Height, terminal_size};

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

	let logged_in;
	let username;

	let mut bone1 = Bone::new(&ip, &port, opt.msgpack);
	bone1.connect();
	
	if let Some(username_tmp) = &opt.username {
		if let Some(password) = &opt.password {
			let result = bone1.login(username_tmp, password);
			match result {
				Err(msg) => panic!("Error while logging in: {}", msg),
				_ => { logged_in = true; username = String::from(username_tmp); },
			}
		} else {
			panic!("--username supplied without --password");
		}
	} else {
		logged_in = false;
		username = String::from("");
	}

	if let Some(command) = &opt.command {
		// command mode
		let command = json::parse(&command).unwrap();
		command_operations(&mut bone1, &command, !opt.no_pretty, atty::is(Stream::Stdout) && opt.response_time, false);
	} else if !atty::is(Stream::Stdin) {
		// pipe mode
		let mut command = String::new();
		stdin().read_line(&mut command).unwrap();

		let command = json::parse(&command).unwrap();
		command_operations(&mut bone1, &command, !opt.no_pretty, atty::is(Stream::Stdout) && opt.response_time, false);
	} else {
		// shell mode
		let data = match bone1.send_command(&json::object!{"command" => "serial_number"}) {
			Ok(n) => n,
			Err(_err) => json::object!{"error" => "missing"},
		};

		let alias = &data["payload"]["alias"];
		let serial_number = &data["payload"]["serial_number"];
		let cnt_str;

		if !alias.is_string() {
			if !serial_number.is_string() {
				cnt_str = String::from("");
			} else {
				cnt_str = serial_number.to_string();
			}
		} else {
			cnt_str = alias.to_string();
		}

		writeln_dimmed(&format!("Connected to {}:{} ({})", ip, port, serial_number.to_string())).unwrap();

		if logged_in {
			writeln_dimmed(&format!("Successfully authenticated as user {}", username)).unwrap();
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

					if command == "login" {
						let mut username = String::new();
						print!("username: ");
						stdout().flush().unwrap();
						stdin().read_line(&mut username).unwrap();
						let tmp_len = username.trim_end().len();
						username.truncate(tmp_len);

						let password = rpassword::read_password_from_tty(Some("password: ")).unwrap();

						let result = bone1.login(&username, &password);
						match result {
							Err(msg) => write_stderr(&format!("Error while logging in: {}", msg)).unwrap(),
							_ => writeln_dimmed(&format!("Successfully authenticated as user {}", username)).unwrap(),
						}

						continue;
					}

					match command.find(" ") {
						Some(n) => {
							let s = command.split_at(n);
							let payload = match json::parse(s.1) {
								Ok(n) => n,
								Err(err) => { write_stderr(&format!("error parsing payload: {}", err)).unwrap(); continue; },
							};
							command = json::object!{"command": s.0.clone(), "payload": payload, "api": opt.api}.dump();
						},
						None => { 
							command = json::object!{"command": command.clone(), "api": opt.api}.dump(); 
						},
					}
				}
			} else {
				continue;
			}

			let result = json::parse(&command);
			match result {
				Err(msg) => write_stderr(&format!("invalid input: {}", msg)).unwrap(),
				Ok(command) => {
					command_operations(&mut bone1, &command, !opt.no_pretty, opt.response_time, true);
				}
			}
		}
	}

	Ok(())
}

fn create_xy<T: Clone>(data: &[T], dt: f32) -> Vec<(f32, T)> {
	let mut out = Vec::new();
	
	for t in 0..data.len() {
		out.push((t as f32 * dt, data[t].clone()));
	}

	out
}

fn command_operations(bone: &mut Bone, command: &json::JsonValue, pretty: bool, response_time: bool, echo_command: bool) {
	let size = terminal_size();
	let term_size = {
		if let Some((Width(w), Height(_h))) = size {
			(w * 2 - 50, 80u16)
		} else {
			(200, 80u16)
		}
	};

	if echo_command {
		writeln_dimmed(&command.dump()).unwrap();
	}

	let start = Instant::now();
	if command["command"] == "sync" {
		let data = bone.send_raw_command(&command).unwrap();
		let duration = start.elapsed().as_millis();

		if response_time {
			writeln_dimmed(&format!("took {} ms", duration)).unwrap();
		}

		let cycle_time = {
			let parsed = bone.send_command(&json::object!{"command" => "cycle_time"});

			match parsed {
				Ok(n) => match n["payload"]["cycle_time"].as_number() {
					Some(n) => f32::from(n) * 1E-6,
					_ => 2E-4,
				},
				Err(_err) => 2E-4,
			}
		};

		for v in &data.1 {
			let mean = statistical::mean(&v.1[..]);
			let stdev = statistical::standard_deviation(&v.1[..], None);

			println!("{} mean = {}, stdev = {}", v.0, mean, stdev);
			Chart::new(term_size.0.into(), term_size.1.into(), 0., data.1[0].1.len() as f32 * cycle_time)
				.lineplot(&Shape::Lines(create_xy(&v.1, cycle_time).as_slice()))
				.nice();
		}
	} else if command["command"] == "dv_data" {
		let data = bone.send_dv_command(&command).unwrap();
		let duration = start.elapsed().as_millis();

		if response_time {
			writeln_dimmed(&format!("took {} ms", duration)).unwrap();
		}

		Chart::new(term_size.0.into(), term_size.1.into(), 0., data.len() as f32 / 10.)
			.lineplot(&Shape::Lines(create_xy(&data, 0.1).as_slice()))
			.nice();
	} else {
		let parsed = bone.send_command(&command).unwrap();
		let duration = start.elapsed().as_millis();

		let pretty_response;

		if pretty {
			pretty_response = json::stringify_pretty(parsed, 4);
		} else {
			pretty_response = json::stringify(parsed);
		}

		if response_time {
			writeln_dimmed(&format!("took {} ms", duration)).unwrap();
		}

		println!("{}", pretty_response);
	}
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

fn write_stderr(output: &str) -> Result<()> {
	let mut stderr = StandardStream::stderr(ColorChoice::Always);
	stderr.set_color(ColorSpec::new().set_fg(Some(Color::Red)).set_bold(true))?;
	match writeln!(&mut stderr, "{}", output) {
		Ok(()) => (),
		Err(_err) => stderr.set_color(&ColorSpec::new())?,
	};
	stderr.set_color(&ColorSpec::new())?;
	Ok(())
}