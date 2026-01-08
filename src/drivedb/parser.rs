use nom::branch::alt;
use nom::bytes::complete::{tag, take_until};
use nom::character::complete::{char, multispace1, none_of, one_of};
use nom::combinator::{eof, map, opt, value};
use nom::multi::{many0, many1};
use nom::sequence::{delimited, preceded, terminated};
use nom::IResult;
use nom::Parser;

fn comment_block(input: &str) -> IResult<&str, ()> {
	let (input, _) = tag("/*")(input)?;
	let (input, _) = take_until("*/")(input)?;
	let (input, _) = tag("*/")(input)?;
	Ok((input, ()))
}

fn comment(input: &str) -> IResult<&str, ()> {
	let (input, _) = tag("//")(input)?;
	let (input, _) = take_until("\n")(input)?;
	let (input, _) = opt(char('\n')).parse(input)?;
	Ok((input, ()))
}

fn whitespace(input: &str) -> IResult<&str, ()> {
	let (input, _) = many0(alt((value((), multispace1), comment, comment_block))).parse(input)?;
	Ok((input, ()))
}

// TODO? \[bfav?0] \ooo \xhh
fn string_escaped_char(input: &str) -> IResult<&str, char> {
	let (input, _) = char('\\')(input)?;
	let (input, c) = one_of("\\\"'nrt")(input)?;
	let c = match c {
		'\\' => '\\',
		'"' => '"',
		'\'' => '\'',
		'n' => '\n',
		'r' => '\r',
		't' => '\t',
		_ => unreachable!(),
	};
	Ok((input, c))
}

fn string_char(input: &str) -> IResult<&str, char> {
	alt((none_of("\n\\\""), string_escaped_char)).parse(input)
}

fn string_literal(input: &str) -> IResult<&str, String> {
	delimited(
		char('"'),
		map(many0(string_char), |s: Vec<char>| s.into_iter().collect()),
		char('"')
	).parse(input)
}

fn string(input: &str) -> IResult<&str, String> {
	let (input, s0) = string_literal(input)?;
	let (input, ss) = many0(preceded(whitespace, string_literal)).parse(input)?;
	let mut s = s0;
	for i in ss {
		s.push_str(i.as_str());
	}
	Ok((input, s))
}

/// drivedb.h entry
#[derive(Debug)]
pub struct Entry {
	/// > Informal string about the model family/series of a device.
	pub family: String,

	/// > POSIX extended regular expression to match the model of a device.
	/// > This should never be "".
	pub model: String,

	/// > POSIX extended regular expression to match a devices's firmware.
	///
	/// Optional if "".
	pub firmware: String,

	/// > A message that may be displayed for matching drives.
	/// > For example, to inform the user that they may need to apply a firmware patch.
	pub warning: String,

	/// > String with vendor-specific attribute ('-v') and firmware bug fix ('-F') options.
	/// > Same syntax as in smartctl command line.
	pub presets: String,
}

fn comma(input: &str) -> IResult<&str, ()> {
	let (input, _) = whitespace(input)?;
	let (input, _) = char(',')(input)?;
	let (input, _) = whitespace(input)?;
	Ok((input, ()))
}

fn entry(input: &str) -> IResult<&str, Entry> {
	let (input, _) = char('{')(input)?;
	let (input, _) = whitespace(input)?;
	let (input, family) = string(input)?;
	let (input, _) = comma(input)?;
	let (input, model) = string(input)?;
	let (input, _) = comma(input)?;
	let (input, firmware) = string(input)?;
	let (input, _) = comma(input)?;
	let (input, warning) = string(input)?;
	let (input, _) = comma(input)?;
	let (input, presets) = string(input)?;
	let (input, _) = whitespace(input)?;
	let (input, _) = char('}')(input)?;
	Ok((
		input,
		Entry {
			family: family,
			model: model,
			firmware: firmware,
			warning: warning,
			presets: presets,
		},
	))
}

pub fn database(input: &str) -> IResult<&str, Vec<Entry>> {
	let (input, _) = whitespace(input)?;
	let (input, entries) = many1(terminated(entry, comma)).parse(input)?;
	let (input, _) = whitespace(input)?;
	let (input, _) = eof(input)?;
	let entries = entries.into_iter()
		.filter(|entry| {
			// > The entry is ignored if [modelfamily] starts with a dollar sign.
			!entry.family.starts_with('$')
		})
		.collect();
	Ok((input, entries))
}
