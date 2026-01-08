#![cfg_attr(feature = "cargo-clippy", allow(print_with_newline))]

#![warn(
	missing_debug_implementations,
	// TODO?..
	//missing_docs,
	//missing_copy_implementations,
	trivial_casts,
	trivial_numeric_casts,
	unsafe_code,
	unstable_features,
	unused_import_braces,
	unused_qualifications,
)]

extern crate hdd;

use hdd::{device, Device};
use hdd::scsi::SCSIDevice;
use hdd::ata::ATADevice;

use hdd::ata::data::id;
use hdd::drivedb;
use hdd::ata::misc::{self, Misc};
use hdd::scsi::ATAError;

use clap::{Arg, ArgAction, Command};
use clap::builder::PossibleValuesParser;

extern crate serde_json;
extern crate separator;
extern crate number_prefix;
extern crate prettytable;

extern crate log;
extern crate env_logger;
use log::LevelFilter;
use env_logger::Builder as LogBuilder;

use std::path::Path;

#[macro_use]
extern crate lazy_static;
mod subcommands;
use crate::subcommands::SUBCOMMANDS;

pub fn when_smart_enabled<F>(status: &id::Ternary, action_name: &str, mut action: F) where F: FnMut() -> () {
	match status {
		id::Ternary::Unsupported => eprint!("S.M.A.R.T. is not supported, cannot show {}\n", action_name),
		id::Ternary::Disabled => eprint!("S.M.A.R.T. is disabled, cannot show {}\n", action_name),
		id::Ternary::Enabled => action(),
	}
}

#[allow(non_upper_case_globals)]
static drivedb_default: [&'static str; 3] = [
	"/var/lib/smartmontools/drivedb/drivedb.h",
	"/usr/local/share/smartmontools/drivedb.h", // for all FreeBSD folks out there
	"/usr/share/smartmontools/drivedb.h",
];
#[allow(non_upper_case_globals)]
static drivedb_additional_default: [&'static str; 1] = [
	"/etc/smart_drivedb.h",
];

/// Returns concatenated list of entries from main and additional drivedb files, falling back to built-in paths if none were provided.
pub fn open_drivedb(options: Option<Vec<String>>) -> Option<drivedb::DriveDB> {
	let options = options.unwrap_or_default();

	let (paths_add, paths_main): (Vec<&str>, Vec<&str>) = options.iter()
		.map(|path| path.as_str())
		.partition(|path| path.starts_with('+'));

	// trim leading '+'
	let paths_add: Vec<&str> = paths_add.iter().map(|path| &path[1..]).collect();

	let mut show_warn_add = true;

	// apply defaults if one of the lists is not provided
	// also silence warnings for default additional file
	let (paths_main, paths_add) = if paths_main.is_empty() {
		let paths_main = drivedb_default.to_vec();

		let paths_add = if paths_add.is_empty() {
			show_warn_add = false;
			drivedb_additional_default.to_vec()
		} else {
			paths_add
		};
		(paths_main, paths_add)
	} else {
		// do not apply defaults to paths_add if paths_main is not the default one
		(paths_main, paths_add)
	};

	let mut loader = drivedb::Loader::new();

	for f in paths_add {
		match loader.load_additional(f) {
			Ok(()) => (),
			Err(e) => if show_warn_add {
				eprint!("Cannot open additional drivedb file {}: {}\n", f, e);
			},
		}
	}

	for f in paths_main {
		match loader.load(f) {
			Ok(()) => {
				break; // we only need one 'main' file, the first valid one
			},
			Err(e) => eprint!("Cannot open drivedb file {}: {}\n", f, e),
		}
	}

	// TODO? show regex error to the world
	loader.db().ok()
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy)]
enum Type { Auto, SAT, SCSI }

#[cfg(target_os = "freebsd")]
#[derive(Debug, Clone, Copy)]
enum Type { Auto, ATA, SAT, SCSI }

impl Type {
	fn variants() -> &'static [&'static str] {
		#[cfg(target_os = "linux")]
		{
			&["auto", "sat", "scsi"]
		}
		#[cfg(target_os = "freebsd")]
		{
			&["auto", "ata", "sat", "scsi"]
		}
	}
}

impl ::std::str::FromStr for Type {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s.to_ascii_lowercase().as_str() {
			"auto" => Ok(Type::Auto),
			"sat" => Ok(Type::SAT),
			"scsi" => Ok(Type::SCSI),
			#[cfg(target_os = "freebsd")]
			"ata" => Ok(Type::ATA),
			_ => Err(()),
		}
	}
}

#[derive(Debug)]
pub enum DeviceArgument {
	#[cfg(not(target_os = "linux"))]
	ATA(ATADevice<Device>, id::Id),
	SAT(ATADevice<SCSIDevice>, id::Id),
	SCSI(SCSIDevice),
}

fn main() {
	let mut log = LogBuilder::new();

	/*
	XXX this bit of clap.rs lets me down
	we want to allow users to type in types in lower case, but .possible_values() would not allow that unless we pass it modified list of values
	so why do we do it here and not in-place?
	- to_ascii_lowercase() returns `String`s, but .possible_values() only accepts `&str`s, so someone needs to own them. Sigh.
	- the result looks somewhat clunky.

	see also https://github.com/kbknapp/clap-rs/issues/891
	*/
	let args = {
		let mut cmd = Command::new("hdd")
		.about("yet another disk querying tool")
		.version(env!("CARGO_PKG_VERSION"))
		.subcommand_required(true)
		.arg(Arg::new("type")
			.short('t')
			.long("type")
			.value_parser(PossibleValuesParser::new(Type::variants()))
			.default_value("auto")
			.help("device type")
		)
		.arg(Arg::new("debug")
			.short('d')
			.long("debug")
			.action(ArgAction::Count)
			.help("Verbose output: set once to log actions, twice to also show raw data buffers\ncan also be set though env_logger's RUST_LOG env")
		)
		/*
		Unlike other pretty common arguments like `--json`, and unlike in tools like `smartctl`, `device` appears before the subcommand.
		Sure this is surprising and "counterintuitive" for users, but there are reasons to do so:
		- if you have to deal with the same disk over and over again for some reason, you're only interested in subcommands, which are easier to edit if they're at the end of your shell prompt (i.e. much less `^W`s and `^[b`s to type),
		- if you want to quickly go through disks in your system, there are better ways to do so anyways:
		  - write a loop (e.g. `for d in /dev/sd?; do hdd $d info; done`),
		  - my guess is you're just interested in disk attributes, in which case you should really be looking into your monitoring system (doesn't matter whether it's local or remote).
		*/
		.arg(Arg::new("device")
			.help("Device to query")
			//.required(true) // optional for 'list' subcommand, required for anything else
			.index(1)
		);
		for subcommand in SUBCOMMANDS.values() {
			cmd = cmd.subcommand(subcommand.subcommand());
		}
		cmd.get_matches()
	};

	if let Ok(var) = std::env::var("RUST_LOG") {
		log.parse_filters(&var);
	}
	// -d takes precedence over RUST_LOG which some might export globally for some reasons
	log.filter(Some("hdd"), {
		use self::LevelFilter::*;
	match args.get_count("debug") {
			0 => Warn,
			1 => Info,
			_ => Debug,
		}
	});
	log.init();

	let path = args.get_one::<String>("device").map(|path| Path::new(path));
	let dev = path.map(|p| Device::open(p).unwrap());

	let dtype = args.get_one::<String>("type")
		.map(|s| s.as_str())
		.unwrap_or("auto")
		.parse::<Type>().unwrap();

	let (subcommand, sargs) = args.subcommand().unwrap();
	// unwrap() ×2: clap should not allow subcommands that do not exist
	let subcommand = SUBCOMMANDS.get(subcommand).unwrap();

	/*
	Why do we issue ATA IDENTIFY DEVICE here?
	- Device id is what every subcommand uses for one reason or the other, but usually to check whether some feature is supported and enabled.
	- It allows us to distinguish between pure SCSI devices and ATA devices behind SAT by issuing ATA PASS-THROUGH and checking whether this command is supported.
	*/

	let dev = dev.map(|dev| match dtype {
		Type::Auto => {
			match dev.get_type().unwrap() {
				device::Type::SCSI => {
					// check whether devices replies to ATA PASS-THROUGH
					let satdev = ATADevice::new(SCSIDevice::new(dev));
					match satdev.get_device_id() {
						// this is really an ATA device
						Ok(id) =>
							DeviceArgument::SAT(satdev, id),
						// nnnnope, plain SCSI
						Err(misc::Error::SCSI(ATAError::NotSupported)) =>
							DeviceArgument::SCSI(satdev.unwrap()),
						// unexpected errors: warn and continue as plain SCSI
						Err(e) => {
							eprint!("ATA PASS-THROUGH probe failed (treating as SCSI): {}\n", e);
							DeviceArgument::SCSI(satdev.unwrap())
						},
					}
				},
				#[cfg(not(target_os = "linux"))]
				device::Type::ATA => {
					let atadev = ATADevice::new(dev);
					let id = atadev.get_device_id().unwrap();
					DeviceArgument::ATA(atadev, id)
				},
			}
		},
		#[cfg(target_os = "freebsd")]
		Type::ATA => {
			let dev = ATADevice::new(dev);
			let id = dev.get_device_id().unwrap();
			DeviceArgument::ATA(dev, id)
		},
		Type::SAT => {
			let dev = ATADevice::new(SCSIDevice::new(dev));
			let id = dev.get_device_id().unwrap();
			DeviceArgument::SAT(dev, id)
		},
		Type::SCSI => DeviceArgument::SCSI(SCSIDevice::new(dev)),
	});

	subcommand.run(&path, &dev.as_ref(), sargs)
}
