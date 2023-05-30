// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{Comedy, Error, LineOpts};
use anyhow::{bail, Context, Result};
use gpiocdev::line::Offset;
use gpiocdev::AbiVersion;
use nohash_hasher::IntMap;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Eq, Hash, PartialEq)]
pub struct ChipOffset {
    // This is the idx into the Vec<ChipInfo>, not a system gpiochip#.
    pub chip_idx: usize,
    pub offset: Offset,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ChipInfo {
    pub path: PathBuf,
    pub name: String,
    pub named_lines: IntMap<Offset, String>,
}

impl ChipInfo {
    pub fn line_name(self: &ChipInfo, offset: &Offset) -> Option<&str> {
        self.named_lines.get(offset).map(|s| s.as_ref())
    }
}

pub struct Resolver {
    pub lines: HashMap<String, ChipOffset>,
    pub chips: Vec<ChipInfo>,
}

impl Resolver {
    pub fn resolve_lines(lines: &[String], opts: &LineOpts, abiv: AbiVersion) -> Result<Resolver> {
        let r = Self::resolve_lines_unvalidated(lines, opts, abiv)?;
        r.validate(lines, opts)?;
        Ok(r)
    }

    pub fn resolve_lines_unvalidated(
        lines: &[String],
        opts: &LineOpts,
        abiv: AbiVersion,
    ) -> Result<Resolver> {
        let chips = match &opts.chip {
            Some(chip_id) => vec![super::chip_path_from_id(chip_id)],
            None => super::all_chip_paths()?,
        };

        let mut r = Resolver {
            lines: HashMap::new(),
            chips: Vec::new(),
        };
        let mut chip_idx = 0;
        for (idx, path) in chips.iter().enumerate() {
            let found_count = r.lines.len();
            let chip = super::chip_from_path(path, abiv)?;
            let kci = chip
                .info()
                .with_context(|| format!("unable to read info from {}", chip.name()))?;
            let mut ci = ChipInfo {
                path: chip.path().to_owned(),
                name: kci.name,
                named_lines: IntMap::default(),
            };
            let mut used_lines = IntMap::default();

            // first match line by offset - but only when id by offset is possible
            if idx == 0 && opts.chip.is_some() && !opts.by_name {
                for id in lines {
                    if let Ok(offset) = id.parse::<u32>() {
                        if offset < kci.num_lines {
                            used_lines.insert(offset, id.to_owned());
                            r.lines
                                .insert(id.to_owned(), ChipOffset { chip_idx, offset });
                        }
                    }
                }
            }
            // match by name
            for offset in 0..kci.num_lines {
                let li = chip.line_info(offset).with_context(|| {
                    format!(
                        "unable to read info for line {} from {}",
                        offset,
                        chip.name()
                    )
                })?;
                for id in lines {
                    if id.as_str() == li.name.as_str() {
                        if !r.lines.contains_key(id) {
                            used_lines.insert(offset, id.to_owned());
                            r.lines
                                .insert(id.to_owned(), ChipOffset { chip_idx, offset });
                            ci.named_lines.insert(offset, id.to_owned());

                            if !opts.strict && r.lines.len() == lines.len() {
                                break;
                            }
                        } else if opts.strict {
                            bail!(Error::NonUniqueLine(id.into()));
                        }
                    }
                }
            }
            if found_count != r.lines.len() {
                r.chips.push(ci);
                chip_idx += 1;
            }
        }
        Ok(r)
    }

    // check that requested lines are found and unique
    pub fn validate(&self, lines: &[String], opts: &LineOpts) -> Result<()> {
        let mut errs = Comedy::new();
        for (idx, id) in lines.iter().enumerate() {
            if let Some(line) = self.lines.get(id) {
                for prev in lines.iter().take(idx) {
                    if let Some(found) = self.lines.get(prev) {
                        if line.chip_idx == found.chip_idx && line.offset == found.offset {
                            errs.push(Error::DuplicateLine(prev.into(), id.into()));
                        }
                    }
                }
            } else if !opts.by_name && id.parse::<u32>().is_ok() && opts.chip.is_some() {
                errs.push(Error::OffsetOutOfRange(
                    id.into(),
                    opts.chip.as_ref().unwrap().into(),
                ));
            } else {
                errs.push(Error::NoSuchLine(id.into()));
            }
        }

        if !errs.is_empty() {
            return Err(errs.into());
        }
        Ok(())
    }
}
