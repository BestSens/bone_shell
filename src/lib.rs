use std::io::prelude::*;
use std::net::TcpStream;

extern crate crypto;
use crypto::digest::Digest;
use crypto::sha2::Sha512;

extern crate rmpv;
extern crate rmp_serde;
extern crate serde_json;

use serde_json::Value;

pub struct Bone {
	ip: String,
	port: String,
	stream: Option<TcpStream>,
	enable_msgpack: bool,
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

	pub fn new(ip: &str, port: &str, enable_msgpack: bool) -> Bone {
		Bone {
			ip: ip.to_string(),
			port: port.to_string(),
			stream: None,
			enable_msgpack: enable_msgpack,
		}
	}

	pub fn connect(&mut self) {
		self.stream = Some(TcpStream::connect(&self.get_connection_string()).unwrap());
	}

	pub fn send_command(&mut self, command: &json::JsonValue) -> json::JsonValue {
		if let Some(ref mut stream) = self.stream {
			let send_data;
			if !self.enable_msgpack {
				let s = String::from(command.dump());
				send_data = s.as_bytes().to_vec();
			} else { 
				let command: Value = serde_json::from_str(&command.dump()).unwrap();
				send_data = rmp_serde::to_vec(&command).unwrap();
			}

			let send_data = [&send_data[..], "\r\n".as_bytes()].concat();

			let mut pos = 0;
			while pos < send_data.len() {
				let bytes_written = stream.write(&send_data[pos..]).unwrap();
				pos += bytes_written;
			}

			let mut buffer = [0; 8];
			stream.read_exact(&mut buffer).unwrap();

			let s = String::from_utf8(buffer.to_vec()).unwrap();
			let response_len = usize::from_str_radix(&s, 16).unwrap();

			let mut buffer = vec![0; response_len];
			let mut t = 0;

			while t < response_len {
				let size = stream.read(&mut buffer[t..]).unwrap();
				t += size;
			}

			if !self.enable_msgpack {
				let response = String::from_utf8(buffer).unwrap();
				json::parse(&response).unwrap()
			} else {
				let value: rmpv::Value = rmp_serde::from_slice(&buffer[..]).unwrap();
				let json = serde_json::to_string(&value).unwrap();
				json::parse(&json).unwrap()
			}
		} else {
			panic!("Not connected");
		}
	}

	pub fn login(&mut self, username: &str, password: &str) -> Result<String, String> {
		let command = json::object!{
			"command" => "request_token"
		};

		let response = self.send_command(&command);
		let token = &response["payload"]["token"].to_string();
		let signed_token = Bone::get_signed_token(&password, &token);

		let command = json::object!{
			"command" => "auth",
			"payload" => json::object!{
				"signed_token" => signed_token,
				"username" => username
			}
		};

		let response = self.send_command(&command);

		let err = response["payload"]["error"].to_string();

		if err != "null" {
			return Err(err)
		}

		Ok(response["payload"]["username"].to_string())
	}
}