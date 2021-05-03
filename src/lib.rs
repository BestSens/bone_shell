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

	fn calc_saw(buffer: &Vec<u8>, output_vec: &mut Vec<(String, Vec<f32>)>) {
		let mut rt_buf: Vec<f32> = Vec::new();
		let mut amp_buf: Vec<f32> = Vec::new();

		for i in (0..buffer.len()).step_by(4) {
			let data: u32 = buffer[i+3] as u32 + 
							((buffer[i+2] as u32) << 8) +
							((buffer[i+1] as u32) << 16) +
							((buffer[i] as u32) << 24);

			let mut runtime: f32 = ((data & 0xfffff000) >> 12) as f32;
			runtime /= 521.0;
			runtime *= 100.0;

			let mut amplitude: f32 = (data & 0x00000fff) as f32;

			amplitude /= 4096.0;
			amplitude *= 5.0;
			amplitude -= 2.5;
			amplitude *= 2.0;

			rt_buf.push(runtime);
			amp_buf.push(amplitude);
		}

		output_vec.push(("rt".to_string(), rt_buf));
		output_vec.push(("amp".to_string(), amp_buf));
	}

	fn calc_f32(buffer: &Vec<u8>, output_vec: &mut Vec<(String, Vec<f32>)>, name: &str) {
		let mut temp: Vec<f32> = Vec::new();

		for i in (0..buffer.len()).step_by(4) {
			let data: u32 = buffer[i+3] as u32 + ((buffer[i+2] as u32) << 8) + ((buffer[i+1] as u32) << 16) + ((buffer[i] as u32) << 24);

			let float: f32 = unsafe {
				std::mem::transmute(data)
			};

			temp.push(float);
		}

		output_vec.push((name.to_string(), temp));
	}

	fn calc_dv(buffer: &Vec<u8>) -> Vec<f32> {
		let mut out = Vec::new();
		for i in (0..buffer.len()).step_by(3) {
			let s = String::from_utf8(buffer[i..i+3].to_vec()).unwrap();
			let dv = usize::from_str_radix(&s, 16).unwrap();
			let dv = (dv as f32 - 2048.) / 4096. * 5.;
			out.push(dv);
		}

		out
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

	pub fn send_raw_command(&mut self, command: &json::JsonValue) -> Result<(i32, Vec<(String, Vec<f32>)>), String> {
		if let Some(ref mut stream) = self.stream {
			let send_data;

			let mut filter: Vec<String> = Vec::new();

			if command["payload"]["filter"].is_array() {
				for a in command["payload"]["filter"].members() {
					filter.push(a.to_string());
				}
			} else {
				filter = vec!(String::from("saw"), String::from("int2"), String::from("coe"), String::from("int"));
			}

			if !self.enable_msgpack {
				let s = String::from(command.dump());
				send_data = s.as_bytes().to_vec();
			} else { 
				let command: Value = match serde_json::from_str(&command.dump()) {
					Ok(n) => n,
					Err(err) => return Err(err.to_string()),
				};
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

			let mut ret_vect = Vec::new();

			let mut last_position = [0; 4];
			stream.read_exact(&mut last_position).unwrap();

			let last_position: i32 = last_position[3] as i32 + 
									((last_position[2] as i32) <<  8) + 
									((last_position[1] as i32) << 16) + 
									((last_position[0] as i32) << 24); 

			let response_len = response_len - 4;

			let mut buffer = vec![0; response_len];
			let mut t = 0;

			while t < response_len {
				let size = stream.read(&mut buffer[t..]).unwrap();
				t += size;
			}

			let split_val = response_len / filter.len();
			let mut pos = 0;

			for current in filter {
				match &current[..] {
					"saw" => Bone::calc_saw(&buffer[pos..pos+split_val].to_vec(), &mut ret_vect),
					_ => Bone::calc_f32(&buffer[pos..pos+split_val].to_vec(), &mut ret_vect, &current),
				}

				pos += split_val;
			}

			Ok((last_position, ret_vect))
		} else {
			panic!("Not connected");
		}
	}

	pub fn send_dv_command(&mut self, command: &json::JsonValue) -> Result<Vec<f32>, String> {
		if let Some(ref mut stream) = self.stream {
			let send_data;

			if !self.enable_msgpack {
				let s = String::from(command.dump());
				send_data = s.as_bytes().to_vec();
			} else { 
				let command: Value = match serde_json::from_str(&command.dump()) {
					Ok(n) => n,
					Err(err) => return Err(err.to_string()),
				};
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

			Ok(Bone::calc_dv(&buffer))
		} else {
			panic!("Not connected");
		}
	}

	pub fn send_command(&mut self, command: &json::JsonValue) -> Result<json::JsonValue, String> {
		if let Some(ref mut stream) = self.stream {
			let send_data;
			if !self.enable_msgpack {
				let s = String::from(command.dump());
				send_data = s.as_bytes().to_vec();
			} else { 
				let command: Value = match serde_json::from_str(&command.dump()) {
					Ok(n) => n,
					Err(err) => return Err(err.to_string()),
				};
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
				match json::parse(&response) {
					Ok(n) => return Ok(n),
					Err(err) => return Err(err.to_string()),
				}
			} else {
				let value: rmpv::Value = rmp_serde::from_slice(&buffer[..]).unwrap();
				let json = serde_json::to_string(&value).unwrap();
				match json::parse(&json) {
					Ok(n) => return Ok(n),
					Err(err) => return Err(err.to_string()),
				}
			}
		} else {
			panic!("Not connected");
		}
	}

	pub fn login(&mut self, username: &str, password: &str) -> Result<String, String> {
		let command = json::object!{
			"command" => "request_token"
		};

		let response = self.send_command(&command)?;
		let token = &response["payload"]["token"].to_string();
		let signed_token = Bone::get_signed_token(&password, &token);

		let command = json::object!{
			"command" => "auth",
			"payload" => json::object!{
				"signed_token" => signed_token,
				"username" => username
			}
		};

		let response = self.send_command(&command)?;

		let err = &response["payload"]["error"];

		if err.is_string() {
			return Err(err.to_string());
		}

		Ok(response["payload"]["username"].to_string())
	}
}