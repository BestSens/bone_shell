use bone_api::Bone;
use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use std::io::*;
use std::path::PathBuf;
use std::time::Instant;
use clap::Parser;

use crossterm::{
	execute,
	style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
	terminal::size,
};

use current_platform::CURRENT_PLATFORM;
use rustyline::{error::ReadlineError, CompletionType, Config, Editor};
use textplots::{Chart, Plot, Shape};

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Opt {
	#[arg(short, long, default_value = "localhost")]
	connect: String,

	#[arg(short, long)]
	port: Option<String>,

	#[arg(long)]
	unencrypted: bool,

	#[arg(short, long)]
	msgpack: bool,

	#[arg(short, long)]
	no_pretty: bool,

	#[arg(short, long)]
	response_time: bool,

	#[arg(long)]
	version: bool,

	#[arg(long)]
	username: Option<String>,

	#[arg(long)]
	password: Option<String>,

	#[arg(long, default_value = "2")]
	api: u32,

	#[arg(long)]
	serial: Option<u32>,

	command: Option<String>,
}

fn main() -> std::io::Result<()> {
	let opt = Opt::parse();

	if opt.version {
		println!("bone_shell version: {}", VERSION);
		return Ok(());
	}

	let ip;

	if let Some(serial) = opt.serial {
		ip = get_ipv6_link_local_from_serial(serial);
	} else {
		ip = opt.connect;
	}

	let unencrypted = if ip == "localhost" {
		true
	} else {
		opt.unencrypted
	};

	let port = if let Some(port) = opt.port {
		port
	} else {
		if unencrypted {
			"6450".into()
		} else {
			"6451".into()
		}
	};

	writeln_dimmed(&format!("Trying to connect to [{}]:{}...", ip, port)).unwrap();

	let logged_in;
	let username;

	let mut bone1 = Bone::new(&ip, &port, opt.msgpack, !unencrypted);
	match bone1.connect() {
		Err(e) => {
			eprintln!("Error connecting to [{ip}]:{port}: {e}");
			std::process::exit(1)
		}
		_ => (),
	}

	if let Some(username_tmp) = &opt.username {
		if let Some(password) = &opt.password {
			let result = bone1.login(username_tmp, password);
			match result {
				Err(msg) => {
					eprintln!("Error while logging in: {msg}");
					std::process::exit(1)
				}
				_ => {
					logged_in = true;
					username = String::from(username_tmp);
				}
			}
		} else {
			eprintln!("--username supplied without --password");
			std::process::exit(1)
		}
	} else {
		logged_in = false;
		username = String::from("");
	}

	if let Some(command) = &opt.command {
		// command mode
		let command = json::parse(&command).unwrap();
		command_operations(
			&mut bone1,
			&command,
			!opt.no_pretty,
			std::io::stdout().is_terminal() && opt.response_time,
			false,
		);
	} else if !std::io::stdin().is_terminal() {
		// pipe mode
		let mut command = String::new();
		stdin().read_line(&mut command).unwrap();

		let command = json::parse(&command).unwrap();
		command_operations(
			&mut bone1,
			&command,
			!opt.no_pretty,
			std::io::stdout().is_terminal() && opt.response_time,
			false,
		);
	} else {
		// shell mode

		let history_path = match dirs::home_dir() {
			Some(home_path) => home_path.join(".bone_shell_history"),
			None => PathBuf::new(),
		};

		let config = Config::builder()
			.history_ignore_space(true)
			.completion_type(CompletionType::List)
			.auto_add_history(false)
			.build();

		let mut rl = Editor::<(), _>::with_config(config).unwrap();

		if let Some(path) = history_path.to_str() {
			match rl.load_history(path) {
				Ok(_) => (),
				Err(_) => (),
			}
		}

		let data = match bone1.send_command(&json::object! {"command" => "serial_number"}) {
			Ok(n) => n,
			Err(_err) => json::object! {"error" => "missing"},
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

		writeln_dimmed(&format!(
			"Connected to [{}]:{} ({})",
			ip,
			port,
			serial_number.to_string()
		))
		.unwrap();

		if logged_in {
			writeln_dimmed(&format!("Successfully authenticated as user {}", username)).unwrap();
		}

		loop {
			let readline = rl.readline(&format!("{} > ", cnt_str));
			let mut command;

			match readline {
				Ok(line) => {
					command = line;
				}
				Err(ReadlineError::Interrupted) => {
					command = "quit".into();
				}
				Err(ReadlineError::Eof) => {
					command = "quit".into();
				}
				Err(err) => {
					println!("Error: {:?}", err);
					break;
				}
			}

			let _ = rl.add_history_entry(command.clone());

			if let Some(first_char) = command.chars().next() {
				if first_char != '{' && first_char != '[' {
					let tmp_len = command.trim_end().len();
					command.truncate(tmp_len);

					if command == "q" || command == "quit" || command == "exit" {
						break;
					}

					if command == "login" {
						let mut username = String::new();
						print!("username: ");
						stdout().flush().unwrap();
						stdin().read_line(&mut username).unwrap();
						let tmp_len = username.trim_end().len();
						username.truncate(tmp_len);

						let password = rpassword::prompt_password("password: ").unwrap();

						let result = bone1.login(&username, &password);
						match result {
							Err(msg) => {
								write_stderr(&format!("Error while logging in: {}", msg)).unwrap()
							}
							_ => writeln_dimmed(&format!(
								"Successfully authenticated as user {}",
								username
							))
							.unwrap(),
						}

						continue;
					}

					match command.find(" ") {
						Some(n) => {
							let s = command.split_at(n);
							let payload = &s.1[1..];

							let command_name = parse_shortcuts(s.0);

							if let Some(first_char) = payload.chars().next() {
								if first_char != '{' && first_char != '[' {
									command = parse_parameters(command_name, payload, opt.api);
								} else {
									let payload = match json::parse(s.1) {
										Ok(n) => n,
										Err(err) => {
											write_stderr(&format!(
												"error parsing payload: {}",
												err
											))
											.unwrap();
											continue;
										}
									};
									command = json::object!{"command": command_name, "payload": payload, "api": opt.api}.dump();
								}
							}
						}
						None => {
							let command_name = parse_shortcuts(&command);
							command =
								json::object! {"command": command_name, "api": opt.api}.dump();
						}
					}
				}
			} else {
				continue;
			}

			let result = json::parse(&command);
			match result {
				Err(msg) => write_stderr(&format!("invalid input: {}", msg)).unwrap(),
				Ok(command) => {
					command_operations(
						&mut bone1,
						&command,
						!opt.no_pretty,
						opt.response_time,
						true,
					);
				}
			}
		}

		if let Some(path) = history_path.to_str() {
			match rl.append_history(path) {
				Ok(_) => (),
				Err(_) => println!("Error saving history to {path}"),
			}
		}
	}

	Ok(())
}

fn get_ipv6_link_local_from_serial(serial: u32) -> String {
	let network_interfaces = NetworkInterface::show().unwrap();

	let mut interface = None;

	if !CURRENT_PLATFORM.to_string().contains("windows") {
		for itf in network_interfaces.iter() {
			let addrs = &itf.addr;
			for addr in addrs.iter() {
				if addr.ip().is_ipv6() && !addr.ip().is_loopback() {
					if addr.ip().to_string().starts_with("fe80::") {
						interface = Some(itf.name.clone());
						break;
					}
				}
			}
		}
	}

	let hex = format!("{:04x}", serial);
	if interface.is_some() {
		format!(
			"fe80::b5:b1ff:fe{}:{}%{}",
			&hex[..2],
			&hex[2..],
			interface.unwrap()
		)
	} else {
		format!("fe80::b5:b1ff:fe{}:{}", &hex[..2], &hex[2..])
	}
}

fn parse_shortcuts(command: &str) -> &str {
	match command {
		"cd" => "channel_data",
		"ca" => "channel_attributes",
		"sn" => "serial_number",
		"bt" => "board_temp",
		&_ => command,
	}
}

fn parse_parameters(command: &str, argument: &str, api: u32) -> String {
	match command {
		"channel_data" | "channel_attributes" => match argument {
			"--all" => json::object! {"command": command, "payload": {"all": true}, "api": api}.dump(),
			"--hidden" => json::object!{"command": command, "payload": {"all": true, "hidden": true}, "api": api}.dump(),
			"--list" => json::object!{"command": command, "payload": {"all": true, "filter": [""], "hidden": true}, "api": api}.dump(),
			&_ => json::object! {"command": command, "payload": {"name": argument}, "api": api}.dump(),
		},
		"sync" | "sync_json" => match argument {
			&_ => json::object! {"command": command, "payload": {"filter": [argument]}, "api": api}.dump(),
		},
		"remove_user" => json::object! {"command": command, "payload": {"username": argument}, "api": api}.dump(),
		&_ => json::object! {"command": command, "api": api}.dump(),
	}
}

fn get_term_size() -> (u32, u32) {
	match size() {
		Ok((w, _)) => (u32::from(w) * 2 - 50, 80u32),
		_ => (200u32, 80u32),
	}
}

fn create_xy<T: Clone>(data: &[T], dt: f32) -> Vec<(f32, T)> {
	let mut out = Vec::new();

	for t in 0..data.len() {
		out.push((t as f32 * dt, data[t].clone()));
	}

	out
}

fn command_operations(
	bone: &mut Bone,
	command: &json::JsonValue,
	pretty: bool,
	response_time: bool,
	echo_command: bool,
) {
	if echo_command {
		writeln_dimmed(&command.dump()).unwrap();
	}

	let start = Instant::now();
	let duration;
	if command["command"] == "sync" {
		let data = bone.send_sync_command(&command).unwrap();
		duration = start.elapsed().as_millis();

		let cycle_time = {
			let parsed = bone.send_command(&json::object! {"command" => "cycle_time"});

			match parsed {
				Ok(n) => match n["payload"]["cycle_time"].as_number() {
					Some(n) => f32::from(n) * 1E-6,
					_ => 2E-4,
				},
				Err(_err) => 2E-4,
			}
		};

		print_raw(&data.1, cycle_time);
	} else if command["command"] == "ks_sync" {
		let data = bone.send_ks_sync_command(&command).unwrap();
		duration = start.elapsed().as_millis();

		let cycle_time = {
			let parsed = bone.send_command(&json::object! {"command" => "ks_cycle_time"});

			match parsed {
				Ok(n) => match n["payload"]["cycle_time"].as_number() {
					Some(n) => f32::from(n) * 1E-6,
					_ => 2E-4,
				},
				Err(_err) => 2E-4,
			}
		};

		print_raw(&data.1, cycle_time);
	} else if command["command"] == "ks" {
		let data = bone.send_ks_command(&command).unwrap();
		duration = start.elapsed().as_millis();

		let cycle_time = {
			let parsed = bone.send_command(&json::object! {"command" => "ks_cycle_time"});

			match parsed {
				Ok(n) => match n["payload"]["cycle_time"].as_number() {
					Some(n) => f32::from(n) * 1E-6,
					_ => 2E-4,
				},
				Err(_err) => 2E-4,
			}
		};

		print_raw(&data.1, cycle_time);
	} else if command["command"] == "dv_data" {
		let term_size = get_term_size();

		let data = bone.send_dv_command(&command).unwrap();
		duration = start.elapsed().as_millis();

		Chart::new(term_size.0, term_size.1, 0., data.len() as f32 / 10.)
			.lineplot(&Shape::Lines(create_xy(&data, 0.1).as_slice()))
			.nice();
	} else {
		let parsed = bone.send_command(&command).unwrap();
		duration = start.elapsed().as_millis();

		let pretty_response;

		if pretty {
			pretty_response = json::stringify_pretty(parsed, 4);
		} else {
			pretty_response = json::stringify(parsed);
		}

		println!("{}", pretty_response);
	}

	if response_time {
		writeln_dimmed(&format!("took {} ms", duration)).unwrap();
	}
}

fn writeln_dimmed(output: &str) -> Result<()> {
	execute!(
		stdout(),
		SetForegroundColor(Color::Rgb {
			r: 150,
			g: 150,
			b: 150
		}),
		SetAttribute(Attribute::Italic),
		Print(format!("# {}\n", output)),
		ResetColor
	)?;

	Ok(())
}

fn write_stderr(output: &str) -> Result<()> {
	execute!(
		stderr(),
		SetForegroundColor(Color::Red),
		SetAttribute(Attribute::Bold),
		Print(format!("{}\n", output)),
		ResetColor
	)?;

	Ok(())
}

fn print_raw(data: &Vec<(String, Vec<f32>)>, cycle_time: f32) {
	let term_size = get_term_size();

	for v in data {
		if v.1.len() > 1 {
			let mean = statistical::mean(&v.1[..]);

			if mean.is_finite() {
				let stdev = statistical::standard_deviation(&v.1[..], None);

				println!("{}: mean = {}, stdev = {}", v.0, mean, stdev);

				Chart::new(
					term_size.0,
					term_size.1,
					0.,
					data[0].1.len() as f32 * cycle_time,
				)
				.lineplot(&Shape::Lines(create_xy(&v.1, cycle_time).as_slice()))
				.nice();
			} else {
				println!("{}: only NaNs returned", v.0);
			}
		} else {
			write_stderr(&format!(
				"{}: not enough data points returned to plot graph",
				v.0
			))
			.unwrap();
		}
	}
}
