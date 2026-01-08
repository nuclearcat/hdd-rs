use hdd::device::list_devices;

use clap::{
	ArgMatches,
	Command,
};

use serde_json;

use crate::DeviceArgument;
use super::{Subcommand, arg_json};

use std::path::Path;

pub struct List {}
impl Subcommand for List {
	fn subcommand(&self) -> Command {
		Command::new("list")
			.about("Lists disk devices")
			.arg(arg_json())
	}

	fn run(
		&self,
		_: &Option<&Path>,
		dev: &Option<&DeviceArgument>,
		args: &ArgMatches,
	) {
		if dev.is_some() {
			// TODO show usage and whatnot
			eprint!("<device> is redundant\n");
			::std::process::exit(1);
		};

		let devs = list_devices().unwrap_or_else(|err| {
			eprint!("Cannot list devices: {}\n", err);
			::std::process::exit(1);
		});

		if args.get_flag("json") {
			print!("{}\n", serde_json::to_string(&devs).unwrap());
		} else {
			for dev in devs {
				print!("{}\n", dev.into_os_string().to_str().unwrap());
			}
		}
	}
}
