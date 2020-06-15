use std::io::prelude::*;
use std::net::TcpStream;

extern crate crypto;
use crypto::digest::Digest;
use crypto::sha2::Sha512;

use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
struct Opt {
	#[structopt(short, long, default_value = "localhost")]
	connect: String,

	#[structopt(short, long, default_value = "6450")]
	port: String,

	#[structopt(long)]
	username: Option<String>,

	#[structopt(long)]
	password: Option<String>,

	command: String
}

struct Bone {
	ip: String,
	port: String,
	stream: Option<TcpStream>,
}

impl Bone {
	fn get_connection_string(&self) -> String {
		let mut connect_str = String::from(&self.ip);
		connect_str.push_str(":");
		connect_str.push_str(&self.port);
		connect_str
	}

	fn get_sha512_string(input_str: &str) -> String {
		let mut hasher = Sha512::new();

		hasher.input_str(input_str);
		hasher.result_str()
	}

	fn get_signed_token(password: &str, token: &str) -> String {
		let password_hashed = Bone::get_sha512_string(&password);

		let mut concat = String::from(&password_hashed);
		concat.push_str(&token);

		Bone::get_sha512_string(&concat)
	}

	pub fn new(ip: &str, port: &str) -> Bone {
		Bone {
			ip: ip.to_string(),
			port: port.to_string(),
			stream: None,
		}
	}

	pub fn connect(&mut self) {
		self.stream = Some(TcpStream::connect(&self.get_connection_string()).unwrap());
	}

	pub fn send_command(&mut self, command: &str) -> json::JsonValue {
		if let Some(ref mut stream) = self.stream {
			let mut send_data = String::from(command);
			send_data.push_str("\r\n");

			let mut pos = 0;
			while pos < send_data.len() {
				let bytes_written = stream.write(&send_data.as_bytes()[pos..]).unwrap();
				pos += bytes_written;
			}

			let mut buffer = [0; 8];
			stream.read_exact(&mut buffer).unwrap();

			let s = String::from_utf8(buffer.to_vec()).unwrap();
			let respone_len = usize::from_str_radix(&s, 16).unwrap();

			let mut buffer = vec![0; respone_len];
			stream.read(&mut buffer[..respone_len]).unwrap();

			let response = String::from_utf8(buffer).unwrap();

			json::parse(&response).unwrap()
		} else {
			panic!("Not connected");
		}
	}

	pub fn login(&mut self, username: &str, password: &str) -> Result<String, String> {
		let command = json::object!{
			"command" => "request_token"
		};

		let response = self.send_command(&command.dump());
		let token = &response["payload"]["token"].to_string();
		let signed_token = Bone::get_signed_token(&password, &token);

		let command = json::object!{
			"command" => "auth",
			"payload" => json::object!{
				"signed_token" => signed_token,
				"username" => username
			}
		};

		let response = self.send_command(&command.dump());

		let err = response["payload"]["error"].to_string();

		if err != "null" {
			return Err(err)
		}

		Ok(response["payload"]["username"].to_string())
	}
}

fn main() -> std::io::Result<()> {
	let opt = Opt::from_args();

	let ip = opt.connect;
	let port = opt.port;
	let command = opt.command;

	let mut bone1 = Bone::new(&ip, &port);
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

	let parsed = bone1.send_command(&command);
	let pretty_response = json::stringify_pretty(parsed, 4);

	println!("{}", pretty_response);

	Ok(())
}
