// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{actual_abi_version, Error, LineOpts, UapiOpts};
use anyhow::anyhow;
use gpiocdev::chip::Chip;
use gpiocdev::line::{Info, Offset};
use gpiocdev::AbiVersion;
use nohash_hasher::IntMap;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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

#[derive(Debug, Default)]
pub struct Resolver {
    /// The first match from id to (chip,offset)
    pub lines: HashMap<String, ChipOffset>,
    /// The info for lines in chip order
    pub info: Vec<LineInfo>,
    // The info for the relevant chips.
    // In strict mode this will be all chips.
    // Otherwise it is the set of chips scanned to locate the lines.
    pub chips: Vec<ChipInfo>,
    // errors detected during resolution
    pub errors: Vec<anyhow::Error>,
    // ABI version being used
    pub abiv: AbiVersion,
}

impl Resolver {
    /// Basic mode to find the (chip,offset) for the lines.
    ///
    /// Does not populate info.
    pub fn resolve_lines(lines: &[String], line_opts: &LineOpts, uapi_opts: &UapiOpts) -> Resolver {
        Self::resolve_lines_with_info(
            lines,
            line_opts,
            uapi_opts,
            line_opts.strict, // generally, strict means exhaustive too
            false,
        )
    }

    /// Core version which can also return the info for lines
    pub fn resolve_lines_with_info(
        lines: &[String],
        line_opts: &LineOpts,
        uapi_opts: &UapiOpts,
        exhaustive: bool,
        with_info: bool,
    ) -> Resolver {
        let mut r = Resolver {
            ..Default::default()
        };
        let chips = match &line_opts.chip {
            Some(chip_id) => match super::chip_lookup_from_id(chip_id) {
                Ok(p) => vec![p],
                Err(e) => {
                    r.errors.push(e);
                    return r;
                }
            },
            None => match super::all_chip_paths() {
                Ok(c) => c,
                Err(e) => {
                    r.errors.push(e);
                    return r;
                }
            },
        };
        match actual_abi_version(uapi_opts) {
            Ok(abiv) => r.abiv = abiv,
            Err(e) => {
                r.errors.push(e);
                return r;
            }
        }
        let mut uniq_lines = lines.to_vec();
        uniq_lines.sort_unstable();
        uniq_lines.dedup();
        let mut chip_idx = 0;
        let done =
            |r: &Resolver| !exhaustive && !uniq_lines.is_empty() && r.lines.len() == lines.len();

        for (idx, path) in chips.iter().enumerate() {
            let mut chip_used = false;
            let chip = match super::chip_from_path(path, r.abiv) {
                Ok(c) => c,
                Err(e) => {
                    r.errors.push(e);
                    continue;
                }
            };
            let kci = match chip.info() {
                Ok(ci) => ci,
                Err(e) => {
                    r.errors.push(
                        anyhow!(e).context(format!("unable to read info from {}", chip.name())),
                    );
                    continue;
                }
            };
            let mut ci = ChipInfo {
                path: chip.path().to_owned(),
                name: kci.name,
                num_lines: kci.num_lines as usize,
                named_lines: IntMap::default(),
            };

            // first match line by offset - but only when id by offset is possible
            let mut offsets = VecDeque::new();
            if idx == 0 && chips.len() == 1 && !line_opts.by_name {
                for id in &uniq_lines {
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
                        r.get_offset_info(&chip, &offsets);
                    }
                    r.chips.push(ci);
                    break;
                }
            }
            // match by name
            for offset in 0..kci.num_lines {
                let li = match chip.line_info(offset) {
                    Ok(li) => li,
                    Err(e) => {
                        r.errors.push(anyhow!(e).context(format!(
                            "unable to read info for line {} from {}",
                            offset,
                            chip.name()
                        )));
                        // give up on the chip
                        break;
                    }
                };
                let mut save_info = uniq_lines.is_empty();
                // save info for id by offset
                if !offsets.is_empty() && offsets[0] == offset {
                    save_info = true;
                    offsets.pop_front();
                }
                for id in &uniq_lines {
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
                        } else if line_opts.strict {
                            r.push_error(Error::NonUniqueLine(id.into()));
                            return r;
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
            r.get_offset_info(&chip, &offsets);

            if chip_used {
                r.chips.push(ci);
                chip_idx += 1;
            }
            if done(&r) {
                break;
            }
        }
        r.validate(lines, &line_opts.chip, line_opts.by_name);
        r
    }

    fn get_offset_info(&mut self, chip: &Chip, offsets: &VecDeque<Offset>) {
        for offset in offsets {
            match chip.line_info(*offset) {
                Ok(li) => {
                    self.info.push(LineInfo {
                        chip: 0,
                        info: li.clone(),
                    });
                }
                Err(e) => {
                    self.errors.push(anyhow!(e).context(format!(
                        "unable to read info for line {} from {}",
                        offset,
                        chip.name()
                    )));
                }
            };
        }
    }

    // check that requested lines are found and unique
    fn validate(&mut self, lines: &[String], chip: &Option<String>, by_name: bool) {
        let mut lines = lines.to_vec();
        lines.sort_unstable();
        let mut lids: HashMap<&String, usize> = HashMap::new();
        for line in &lines {
            if let Some(lid) = lids.get_mut(line) {
                *lid += 1
            } else {
                lids.insert(line, 1);
            }
        }
        for (id, count) in lids {
            if count > 1 {
                self.push_error(Error::RepeatedLine(id.into()));
            }
        }
        lines.dedup();
        for (idx, id) in lines.iter().enumerate() {
            if let Some(&line) = self.lines.get(id) {
                for prev in lines.iter().take(idx) {
                    if let Some(found) = self.lines.get(prev) {
                        if line.chip_idx == found.chip_idx && line.offset == found.offset {
                            self.push_error(Error::DuplicateLine(prev.into(), id.into()));
                        }
                    }
                }
            } else if !by_name && id.parse::<u32>().is_ok() && chip.is_some() {
                self.push_error(Error::OffsetOutOfRange(
                    id.into(),
                    chip.as_ref().unwrap().into(),
                ));
            } else {
                self.push_error(Error::NoSuchLine(id.into()));
            }
        }
    }

    fn push_error(&mut self, e: Error) {
        self.errors.push(anyhow!(e))
    }
}
