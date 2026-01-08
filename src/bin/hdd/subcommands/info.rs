use hdd::ata::data::id;
use hdd::drivedb;
use hdd::scsi::SCSICommon;
use hdd::scsi::data::inquiry;

use clap::{
	ArgMatches,
	Command,
};

use serde_json;

use separator::Separatable;
use number_prefix::NumberPrefix;

use crate::{DeviceArgument, open_drivedb};
use super::{Subcommand, arg_json, arg_drivedb};

use std::path::Path;

fn bool_to_sup(b: bool) -> &'static str {
	if b { "supported" }
	else { "not supported" }
}

fn ternary_feature_status(x: &id::Ternary) -> &'static str {
	match x {
		id::Ternary::Unsupported => "Unavailable",
		id::Ternary::Disabled => "Disabled",
		id::Ternary::Enabled => "Enabled",
	}
}

fn ata_security_status(state: u16, master_password_id: u16) -> String {
	if (state & 0x0001) == 0 {
		return "Unavailable".to_string();
	}

	let mut out = String::new();
	if (state & 0x0002) == 0 {
		out.push_str("Disabled, ");
		if (state & 0x0008) == 0 {
			out.push_str("NOT FROZEN [SEC1]");
		} else {
			out.push_str("frozen [SEC2]");
		}
	} else {
		out.push_str("ENABLED, PW level ");
		if (state & 0x0100) == 0 {
			out.push_str("HIGH");
		} else {
			out.push_str("MAX");
		}

		if (state & 0x0004) == 0 {
			out.push_str(", not locked, ");
			if (state & 0x0008) == 0 {
				out.push_str("not frozen [SEC5]");
			} else {
				out.push_str("frozen [SEC6]");
			}
		} else {
			out.push_str(", **LOCKED** [SEC4]");
			if (state & 0x0010) != 0 {
				out.push_str(", PW ATTEMPTS EXCEEDED");
			}
		}
	}

	if 0x0000 < master_password_id && master_password_id < 0xfffe {
		out.push_str(&format!(", Master PW ID: 0x{:04x}", master_password_id));
	}

	out
}

fn print_ata_id(id: &id::Id, meta: &Option<drivedb::DriveMeta>) {
	if id.incomplete { print!("WARNING: device reports information it provides is incomplete\n\n"); }

	// XXX id.is_ata is deemed redundant and is skipped
	// XXX we're skipping id.commands_supported for now as it is hardly of any interest to users

	print!("Model:    {}\n", id.model);
	match id.rpm {
		id::RPM::Unknown => (),
		id::RPM::NonRotating => print!("RPM:      N/A (SSD or other non-rotating media)\n"),
		id::RPM::RPM(i) => print!("RPM:      {}\n", i),
	};
	print!("Firmware: {}\n", id.firmware);
	print!("Serial:   {}\n", id.serial);
	// TODO: id.wwn_supported is cool, but actual WWN ID is better

	if let Some(meta) = meta {
		if let Some(family) = meta.family {
			print!("Model family according to drive database:\n  {}\n", family);
		} else {
			print!("This drive is not in the drive database\n");
		}
		if let Some(warning) = meta.warning {
			print!("\n══════ WARNING ══════\n{}\n═════════════════════\n", warning);
		}
	}

	print!("\n");

	print!("Capacity: {} bytes\n", id.capacity.separated_string());
	print!("          ({}, {})\n",
		match NumberPrefix::decimal(id.capacity as f64) {
			NumberPrefix::Prefixed(p, x) => format!("{:.1} {}B", x, p),
			NumberPrefix::Standalone(x)  => format!("{} bytes", x),
		},
		match NumberPrefix::binary(id.capacity as f64) {
			NumberPrefix::Prefixed(p, x) => format!("{:.1} {}B", x, p),
			NumberPrefix::Standalone(x)  => format!("{} bytes", x),
		},
	);
	print!("Sector size (logical):  {}\n", id.sector_size_log);
	print!("Sector size (physical): {}\n", id.sector_size_phy);

	print!("\n");

	print!("ATA version:\n{}\n", id.ata_version.unwrap_or("unknown"));
	if id.sata_version.is_some() || id.sata_speed_max.is_some() || id.sata_speed_current.is_some() {
		let mut details = id.sata_version.unwrap_or("SATA").to_string();
		if let Some(max) = id.sata_speed_max {
			details.push_str(&format!(", {}", max));
		}
		if let Some(current) = id.sata_speed_current {
			details.push_str(&format!(" (current: {})", current));
		}
		print!("SATA Version:\n{}\n", details);
	}
	print!(
		"TRIM Command:     {}\n",
		if !id.trim_supported {
			"not supported".to_string()
		} else {
			let mut details = vec!["Available"];
			if id.trim_deterministic {
				details.push("deterministic");
			}
			if id.trim_zeroed {
				details.push("zeroed");
			}
			details.join(", ")
		}
	);

	print!("\n");

	// The following guide, when printed, is exactly 80 characters
	// ... "..............................................................supported disabled\n"
	print!("Host protected area:           {}\n", id.hpa);
	print!("SMART support is:              {}\n", ternary_feature_status(&id.smart));
	print!("AAM feature is:                {}\n", ternary_feature_status(&id.aam));
	print!("APM feature is:                {}\n", ternary_feature_status(&id.apm));
	print!("Rd look-ahead is:              {}\n", ternary_feature_status(&id.read_look_ahead));
	print!("Write cache is:                {}\n", ternary_feature_status(&id.write_cache));
	print!(
		"DSN feature is:                {}\n",
		if id.dsn_available {
			if id.dsn_enabled { "Enabled" } else { "Disabled" }
		} else {
			"Unavailable"
		}
	);
	print!("ATA Security is:               {}\n", ata_security_status(id.security_state, id.security_master_pw_id));
	print!(
		"Wt Cache Reorder:              {}\n",
		if id.sct_feature_control_supported {
			"Unknown (SCT Feature Control not implemented)"
		} else {
			"Unavailable"
		}
	);
	print!("Power management:              {}\n", bool_to_sup(id.power_mgmt_supported));
	print!("General purpose logging:       {}\n", bool_to_sup(id.gp_logging_supported));
	print!("Trusted computing:             {}\n", bool_to_sup(id.trusted_computing_supported));

	print!("\n");

	print!("Error logging: {}\n", bool_to_sup(id.smart_error_logging_supported));
	print!("Self-test:     {}\n", bool_to_sup(id.smart_self_test_supported));

	print!("\n");
}

fn print_scsi_id(inquiry: &inquiry::Inquiry) {
	print!("Vendor:   {}\n", inquiry.vendor_id);
	print!("Model:    {}\n", inquiry.product_id);
	print!("Firmware: {}\n", inquiry.product_rev);

	// TODO other inquiry fields, capacity, …
}

pub struct Info {}
impl Subcommand for Info {
	fn subcommand(&self) -> Command {
		Command::new("info")
			.about("Prints a basic information about the device")
			.arg(arg_json())
			.arg(arg_drivedb())
	}

	fn run(
		&self,
		_: &Option<&Path>,
		dev: &Option<&DeviceArgument>,
		args: &ArgMatches,
	) {
		let dev = dev.unwrap_or_else(|| {
			// TODO show usage and whatnot
			eprint!("<device> is required\n");
			::std::process::exit(1);
		});

		let ata_id = match dev {
			#[cfg(not(target_os = "linux"))]
			DeviceArgument::ATA(_, id) => Some(id),
			DeviceArgument::SAT(_, id) => Some(id),
			DeviceArgument::SCSI(_) => None,
		};

		let use_json = args.get_flag("json");

		if let DeviceArgument::SCSI(dev) = dev {
			let (_sense, data) = dev.scsi_inquiry(false, 0).unwrap();
			let inquiry = inquiry::parse_inquiry(&data);

			if use_json {
				let info = serde_json::to_value(&inquiry).unwrap();
				print!("{}\n", serde_json::to_string(&info).unwrap());
			} else {
				print_scsi_id(&inquiry);
			}
		}

		if let Some(id) = ata_id {
			let drivedb = open_drivedb(args.get_many::<String>("drivedb")
				.map(|vals| vals.map(|v| v.to_string()).collect()));
			let meta = drivedb.as_ref().map(|drivedb| drivedb.render_meta(
				&id,
				// no need to parse custom vendor attributes,
				// we're only using drivedb for the family and the warning here
				&vec![],
			));

			if use_json {
				let mut info = serde_json::to_value(&id).unwrap();

				if let Some(meta) = &meta {
					if let Some(family) = meta.family {
						info.as_object_mut().unwrap().insert(
							"family".to_string(),
							serde_json::to_value(family).unwrap(),
						);
					}
					if let Some(warning) = meta.warning {
						info.as_object_mut().unwrap().insert(
							"warning".to_string(),
							serde_json::to_value(warning).unwrap(),
						);
					}
				}

				print!("{}\n", serde_json::to_string(&info).unwrap());
			} else {
				print_ata_id(&id, &meta);
			}
		}
	}
}
