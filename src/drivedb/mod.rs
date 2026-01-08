/*!
Use this module to match hard drive and SMART values it returns against smartmontools database.

## Example

```no_run
use hdd::drivedb::{Loader, vendor_attribute};
use hdd::ata::data::id;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut loader = Loader::new();
// look for version updated with `update-smart-drivedb(8)` first
loader.load("/var/lib/smartmontools/drivedb/drivedb.h")
	.or_else(|_| loader.load("/usr/share/smartmontools/drivedb.h"))?;
// `?` is optional though: if nothing can be loaded, loader will still provide dummy db for us

let db = loader.db()?;

// extra attribute definitions that user might give
let user_attributes = vec!["9,minutes"]
	.into_iter()
	.map(|attr| vendor_attribute::parse(attr).unwrap())
	.collect::<Vec<_>>();

// TODO: issue ATA IDENTIFY DEVICE cmd and parse the answer here
let id: id::Id = unimplemented!();

let meta = db.render_meta(&id, &user_attributes);

if let Some(warn) = meta.warning {
	println!("WARNING: {}", warn);
}

if let Some(attr) = meta.render_attribute(9) {
	if let Some(name) = attr.name {
		println!("Attribute 9 is named {}", name);
	}
}
# Ok(())
# }
```
*/

mod parser;
mod presets;
mod drivedb;
mod loader;
pub mod vendor_attribute;
pub use self::vendor_attribute::Attribute;
pub use self::drivedb::{DriveDB, DriveMeta};
pub use self::loader::{Loader, Error};
