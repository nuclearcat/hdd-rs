mod info;
mod health;
mod attrs;
mod list;

use std::collections::HashMap;
use clap::{self, Arg, ArgAction, ArgMatches, Command};
use crate::DeviceArgument;
use std::path::Path;

pub fn arg_json() -> Arg {
	Arg::new("json")
		.long("json")
		.action(ArgAction::SetTrue)
		.help("Export data in JSON")
}

pub fn arg_drivedb() -> Arg {
	Arg::new("drivedb")
			.short('B') // smartctl-like
			.long("drivedb") // smartctl-like
			.num_args(1)
			.action(ArgAction::Append)
			.value_name("[+]FILE")
			/*
			TODO show what default values are; now it's not possible, temporary value [0] is short-living and `.help()` only accepts &str, not String
			[0]	format!("…\ndefault:\n{}\n{}",
					drivedb_default.join("\n"),
					drivedb_additional.iter().map(|i| format!("+{}", i)).collect::<Vec<_>>().join("\n"),
				)
			*/
			.help("paths to drivedb files to look for\nuse 'FILE' for main (system-wide) file, '+FILE' for additional entries\nentries are looked up in every additional file in order of their appearance, then in the first valid main file, stopping at the first match\n(this option and its behavior is, to some extent, consistent with '-B' from smartctl)")
}

pub trait Subcommand: Sync {
	fn subcommand(&self) -> Command;
	fn run(&self, path: &Option<&Path>, dev: &Option<&DeviceArgument>, args: &ArgMatches);
}

static HEALTH: health::Health = health::Health {};
static LIST: list::List = list::List {};
static INFO: info::Info = info::Info {};
static ATTRS: attrs::Attrs = attrs::Attrs {};

lazy_static! {
	pub static ref SUBCOMMANDS: HashMap<&'static str, &'static dyn Subcommand> = {
		let mut m: HashMap<&'static str, &'static dyn Subcommand> = HashMap::new();
		m.insert("health", &HEALTH);
		m.insert("list",   &LIST);
		m.insert("info",   &INFO);
		m.insert("attrs",  &ATTRS);
		m
	};
}
