// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{Comedy, Error, LineOpts};
use anyhow::{bail, Context, Result};
use gpiocdev::chip::Chip;
use gpiocdev::line::{Info, Offset};
use gpiocdev::AbiVersion;
use nohash_hasher::IntMap;
use std::collections::{HashMap, VecDeque};
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
    pub num_lines: usize,
    named_lines: IntMap<Offset, String>,
}

impl ChipInfo {
    pub fn line_name(self: &ChipInfo, offset: &Offset) -> Option<&str> {
        self.named_lines.get(offset).map(|s| s.as_ref())
    }
}

#[derive(Debug)]
pub struct LineInfo {
    pub chip: usize,
    pub info: Info,
}

#[derive(Debug)]
pub struct Resolver {
    /// The first match from id to (chip,offset)
    pub lines: HashMap<String, ChipOffset>,
    /// The info for lines in chip order
    pub info: Vec<LineInfo>,
    // The info for the relevant chips.
    // In strict mode this will be all chips.
    // Otherwise it is the set of chips scanned to locate the lines.
    pub chips: Vec<ChipInfo>,
}

impl Resolver {
    /// Basic mode to find the (chip,offset) for the lines.
    ///
    /// Does not populate info.
    pub fn resolve_lines(lines: &[String], opts: &LineOpts, abiv: AbiVersion) -> Result<Resolver> {
        let chips = match &opts.chip {
            Some(chip_id) => vec![super::chip_path_from_id(chip_id)],
            None => super::all_chip_paths()?,
        };
        let r = Self::resolve_lines_unvalidated(
            lines,
            &chips,
            abiv,
            opts.strict,
            opts.strict, // generally, strict means exhaustive
            opts.by_name,
            false,
        )?;
        r.validate(lines, &opts.chip, opts.by_name)?;
        Ok(r)
    }

    /// Core version which can also return the info for lines
    pub fn resolve_lines_unvalidated(
        lines: &[String],
        chips: &[PathBuf],
        abiv: AbiVersion,
        strict: bool,
        exhaustive: bool,
        by_name: bool,
        with_info: bool,
    ) -> Result<Resolver> {
        let mut lines = lines.to_vec();
        lines.sort_unstable();
        lines.dedup();
        let mut r = Resolver {
            lines: HashMap::new(),
            info: Vec::new(),
            chips: Vec::new(),
        };
        let mut chip_idx = 0;
        let done = |r: &Resolver| !exhaustive && !lines.is_empty() && r.lines.len() == lines.len();

        for (idx, path) in chips.iter().enumerate() {
            let mut chip_used = false;
            let chip = super::chip_from_path(path, abiv)?;
            let kci = chip
                .info()
                .with_context(|| format!("unable to read info from {}", chip.name()))?;
            let mut ci = ChipInfo {
                path: chip.path().to_owned(),
                name: kci.name,
                num_lines: kci.num_lines as usize,
                named_lines: IntMap::default(),
            };

            // first match line by offset - but only when id by offset is possible
            let mut offsets = VecDeque::new();
            if idx == 0 && chips.len() == 1 && !by_name {
                for id in &lines {
                    if let Ok(offset) = id.parse::<u32>() {
                        if offset < kci.num_lines {
                            r.lines
                                .insert(id.to_owned(), ChipOffset { chip_idx, offset });
                            if with_info {
                                offsets.push_back(offset);
                                chip_used = true;
                            }
                        }
                    }
                }
                if done(&r) {
                    if with_info {
                        r.get_offset_info(&chip, &offsets)?;
                    }
                    r.chips.push(ci);
                    break;
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
                let mut save_info = lines.is_empty();
                // save info for id by offset
                if !offsets.is_empty() && offsets[0] == offset {
                    save_info = true;
                    offsets.pop_front();
                }
                for id in &lines {
                    if id.as_str() == li.name.as_str() {
                        save_info = true;
                        if !r.lines.contains_key(id) {
                            chip_used = true;
                            r.lines
                                .insert(id.to_owned(), ChipOffset { chip_idx, offset });

                            ci.named_lines.insert(offset, id.to_owned());
                            if done(&r) {
                                break;
                            }
                        } else if strict {
                            bail!(Error::NonUniqueLine(id.into()));
                        }
                    }
                }
                if with_info && save_info {
                    r.info.push(LineInfo {
                        chip: chip_idx,
                        info: li,
                    });
                    chip_used = true;
                }
            }
            // might still have some offsets that need info, so fill those in
            r.get_offset_info(&chip, &offsets)?;

            if chip_used {
                r.chips.push(ci);
                chip_idx += 1;
            }
            if done(&r) {
                break;
            }
        }
        Ok(r)
    }

    fn get_offset_info(&mut self, chip: &Chip, offsets: &VecDeque<Offset>) -> Result<()> {
        for offset in offsets {
            let li = chip.line_info(*offset).with_context(|| {
                format!(
                    "unable to read info for line {} from {}",
                    offset,
                    chip.name()
                )
            })?;
            self.info.push(LineInfo {
                chip: 0,
                info: li.clone(),
            });
        }
        Ok(())
    }
    // check that requested lines are found and unique
    pub fn validate(&self, lines: &[String], chip: &Option<String>, by_name: bool) -> Result<()> {
        let mut lines = lines.to_vec();
        let mut errs = Comedy::new();
        lines.sort_unstable();
        let mut lids : HashMap<&String, usize> = HashMap::new();
        for line in &lines {
            if let Some(lid) = lids.get_mut(line) {
                *lid += 1
            } else {
                lids.insert(line, 1);
            }
        }
        for (id, count) in lids {
            if count > 1 {
                errs.push(Error::RepeatedLine(id.into()));
            }
        }
        lines.dedup();
        for (idx, id) in lines.iter().enumerate() {
            if let Some(line) = self.lines.get(id) {
                for prev in lines.iter().take(idx) {
                    if let Some(found) = self.lines.get(prev) {
                        if line.chip_idx == found.chip_idx && line.offset == found.offset {
                            errs.push(Error::DuplicateLine(prev.into(), id.into()));
                        }
                    }
                }
            } else if !by_name && id.parse::<u32>().is_ok() && chip.is_some() {
                errs.push(Error::OffsetOutOfRange(
                    id.into(),
                    chip.as_ref().unwrap().into(),
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
