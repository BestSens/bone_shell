use std::env;
use std::io::prelude::*;
use std::net::TcpStream;
use sha2::{Sha512, Digest};

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

	// fn get_hashed_password(password: &str) {
	// 	let mut hasher = Sha512::new();

	// 	hasher.update(password);

	// 	let result = hasher.finalize();

	// 	result
	// }

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

	pub fn login(&mut self, username: &str, password: &str) {
		let token_response = self.send_command("{\"command\":\"request_token\"}");
		let token = &token_response["payload"]["token"];

		println!("token: {}", token);
	}
}

fn main() -> std::io::Result<()> {
	let args: Vec<String> = env::args().collect();
	let ip = &args[1];
	let port = &args[2];
	let command = &args[3];

	let mut bone1 = Bone::new(&ip, &port);
	bone1.connect();
	bone1.login("admin", "1102snestseb");

	let parsed = bone1.send_command(&command);
	let pretty_response = json::stringify_pretty(parsed, 4);

	println!("{}", pretty_response);

	Ok(())
}
