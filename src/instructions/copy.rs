// (C) Copyright 2019-2020 Hewlett Packard Enterprise Development LP

use std::convert::TryFrom;

use snafu::ensure;

use crate::dockerfile_parser::Instruction;
use crate::parser::{Pair, Rule};
use crate::{Span, error::*};

/// A key/value pair passed to a `COPY` instruction as a flag.
///
/// Examples include: `COPY --from=foo /to /from`
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CopyFlag {
  pub span: Span,
  pub name: String,
  pub name_span: Span,
  pub value: String,
  pub value_span: Span
}

impl CopyFlag {
  fn from_record(record: Pair) -> Result<CopyFlag> {
    let span = Span::from_pair(&record);
    let mut name = None;
    let mut value = None;

    for field in record.into_inner() {
      match field.as_rule() {
        Rule::copy_flag_name => name = Some((
          field.as_str().to_string(),
          Span::from_pair(&field)
        )),
        Rule::copy_flag_value => value = Some((
          field.as_str().to_string(),
          Span::from_pair(&field)
        )),
        _ => return Err(unexpected_token(field))
      }
    }

    let (name, name_span) = name.ok_or_else(|| Error::GenericParseError {
      message: "copy flags require a key".into(),
    })?;

    let (value, value_span) = value.ok_or_else(|| Error::GenericParseError {
      message: "copy flags require a value".into()
    })?;

    Ok(CopyFlag {
      span, name, name_span, value, value_span
    })
  }
}

/// A Dockerfile [`COPY` instruction][copy].
///
/// [copy]: https://docs.docker.com/engine/reference/builder/#copy
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CopyInstruction {
  pub span: Span,
  pub flags: Vec<CopyFlag>,
  pub sources: Vec<String>,
  pub destination: String
}

impl CopyInstruction {
  pub(crate) fn from_record(record: Pair) -> Result<CopyInstruction> {
    let span = Span::from_pair(&record);
    let mut flags = Vec::new();
    let mut paths = Vec::new();

    for field in record.into_inner() {
      match field.as_rule() {
        Rule::copy_flag => flags.push(CopyFlag::from_record(field)?),
        Rule::copy_pathspec => paths.push(field.as_str().to_string()),
        Rule::comment => continue,
        _ => return Err(unexpected_token(field))
      }
    }

    ensure!(
      paths.len() >= 2,
      GenericParseError {
        message: "copy requires at least one source and a destination"
      }
    );

    // naughty unwrap, but we know there's something to pop
    let destination = paths.pop().unwrap();

    Ok(CopyInstruction {
      span,
      flags,
      sources: paths,
      destination
    })
  }
}

impl<'a> TryFrom<&'a Instruction> for &'a CopyInstruction {
  type Error = Error;

  fn try_from(instruction: &'a Instruction) -> std::result::Result<Self, Self::Error> {
    if let Instruction::Copy(c) = instruction {
      Ok(c)
    } else {
      Err(Error::ConversionError {
        from: format!("{:?}", instruction),
        to: "CopyInstruction".into()
      })
    }
  }
}

#[cfg(test)]
mod tests {
  use indoc::indoc;

  use super::*;
  use crate::test_util::*;

  #[test]
  fn copy_basic() -> Result<()> {
    assert_eq!(
      parse_single("copy foo bar", Rule::copy)?,
      CopyInstruction {
        span: Span { start: 0, end: 12 },
        flags: vec![],
        sources: strings(&["foo"]),
        destination: "bar".to_string(),
      }.into()
    );

    Ok(())
  }

  #[test]
  fn copy_multiple_sources() -> Result<()> {
    assert_eq!(
      parse_single("copy foo bar baz qux", Rule::copy)?,
      CopyInstruction {
        span: Span { start: 0, end: 20 },
        flags: vec![],
        sources: strings(&["foo", "bar", "baz"]),
        destination: "qux".into(),
      }.into()
    );

    Ok(())
  }

  #[test]
  fn copy_multiline() -> Result<()> {
    // multiline is okay; whitespace on the next line is optional
    assert_eq!(
      parse_single("copy foo \\\nbar", Rule::copy)?,
      CopyInstruction {
        span: Span { start: 0, end: 14 },
        flags: vec![],
        sources: strings(&["foo"]),
        destination: "bar".into(),
      }.into()
    );

    // newlines must be escaped
    assert_eq!(
      parse_single("copy foo\nbar", Rule::copy).is_err(),
      true
    );

    Ok(())
  }

  #[test]
  fn copy_flags() -> Result<()> {
    assert_eq!(
      parse_single(
        "copy --from=alpine:3.10 /usr/lib/libssl.so.1.1 /tmp/",
        Rule::copy
      )?,
      CopyInstruction {
        span: Span { start: 0, end: 52 },
        flags: vec![
          CopyFlag {
            span: Span { start: 5, end: 23 },
            name: "from".into(),
            name_span: Span { start: 7, end: 11 },
            value: "alpine:3.10".into(),
            value_span: Span { start: 12, end: 23 }
          }
        ],
        sources: strings(&["/usr/lib/libssl.so.1.1"]),
        destination: "/tmp/".into(),
      }.into()
    );

    Ok(())
  }

  #[test]
  fn copy_comments() -> Result<()> {
    assert_eq!(
      parse_single(
        indoc!(r#"
          copy \
            --from=alpine:3.10 \

            # hello

            /usr/lib/libssl.so.1.1 \
            # world
            /tmp/
        "#),
        Rule::copy
      )?.into_copy().unwrap(),
      CopyInstruction {
        span: Span { start: 0, end: 86 },
        flags: vec![
          CopyFlag {
            span: Span { start: 9, end: 27 },
            name: "from".into(),
            name_span: Span { start: 11, end: 15 },
            value: "alpine:3.10".into(),
            value_span: Span { start: 16, end: 27 }
          }
        ],
        sources: strings(&["/usr/lib/libssl.so.1.1"]),
        destination: "/tmp/".into(),
      }.into()
    );

    Ok(())
  }
}
