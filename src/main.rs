use std::io::*;
use structopt::StructOpt;
use bone_api::Bone;
use atty::Stream;

#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
pub struct Opt {
	#[structopt(short, long, default_value = "localhost")]
	connect: String,

	#[structopt(short, long, default_value = "6450")]
	port: String,

	#[structopt(short = "m", long)]
	msgpack: bool,

	#[structopt(long)]
	username: Option<String>,

	#[structopt(long)]
	password: Option<String>,

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
		let parsed = bone1.send_command(&json::parse(&command).unwrap());
		let pretty_response = json::stringify_pretty(parsed, 4);

		println!("{}", pretty_response);
	} else if !atty::is(Stream::Stdin) {
		// pipe mode
		let mut command = String::new();
		stdin().read_line(&mut command).unwrap();

		let parsed = bone1.send_command(&json::parse(&command).unwrap());
		let pretty_response = json::stringify_pretty(parsed, 4);

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

			let tmp_len = command.trim_end().len();
			command.truncate(tmp_len);

			if command == "q" || command == "quit" || command == "exit" {
				return Ok(())
			}

			let result = json::parse(&command);
			match result {
				Err(msg) => eprintln!("invalid input: {}", msg),
				Ok(command) => {
					let parsed = bone1.send_command(&command);
					let parsed = match bone1.send_command(&command) {
						Ok(n) => n,
						Err(err) => {eprintln!("Error: {}", err); continue;},
					};
					let pretty_response = json::stringify_pretty(parsed, 4);

					println!("{}", pretty_response);
				}
			}
		}
	}

	Ok(())
}
