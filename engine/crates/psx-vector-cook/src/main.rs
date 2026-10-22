//! `psx-svg-cook`: cook an SVG to a PSVG asset, rasterise one on the host, or
//! diff two PPM frames.
//!
//! ```text
//! psx-svg-cook cook <in.svg> -o <out.psvg> [--tolerance PX] [--budget total=N]
//!                   [--budget GROUP=N] [--budget-per-group N] [--quiet]
//! psx-svg-cook raster <in.psvg> -o <out.ppm> [--groups a,b] [--origin X,Y]
//!                   [--size W,H] [--bg RRGGBB] [--scale N]   (.png or .ppm by extension)
//! psx-svg-cook diff <a.ppm> <b.ppm> [--threshold N] [--region X0,Y0,X1,Y1]
//! ```

use std::process::ExitCode;

use psx_vector::{Placement, Style, VectorAsset};
use psx_vector_cook::raster::{compare, draw_group, AlignedAsset, Image};
use psx_vector_cook::{cook, Budget, CookOptions};

fn pair(s: &str) -> Result<Vec<i64>, String> {
    s.split(',')
        .map(|v| {
            v.trim()
                .parse::<i64>()
                .map_err(|_| format!("bad number `{v}`"))
        })
        .collect()
}

fn run(args: &[String]) -> Result<(), String> {
    let cmd = args
        .first()
        .map(String::as_str)
        .ok_or("missing command (cook, raster, diff)")?;
    let mut positional = Vec::new();
    let mut flags: Vec<(String, String)> = Vec::new();
    let mut it = args[1..].iter();
    while let Some(a) = it.next() {
        if let Some(name) = a.strip_prefix("--").or_else(|| a.strip_prefix('-')) {
            if name == "quiet" {
                flags.push((name.to_string(), String::new()));
            } else {
                let v = it.next().ok_or(format!("{a} needs a value"))?;
                flags.push((name.to_string(), v.clone()));
            }
        } else {
            positional.push(a.clone());
        }
    }
    let flag = |n: &str| {
        flags
            .iter()
            .rev()
            .find(|(k, _)| k == n)
            .map(|(_, v)| v.as_str())
    };
    match cmd {
        "cook" => {
            let input = positional.first().ok_or("missing input.svg")?;
            let out = flag("o").ok_or("missing -o <out.psvg>")?;
            let svg = std::fs::read_to_string(input).map_err(|e| format!("{input}: {e}"))?;
            let mut opts = CookOptions::default();
            if let Some(t) = flag("tolerance") {
                opts.tolerance_px = t.parse().map_err(|_| "bad --tolerance")?;
            }
            let mut budget = Budget::default();
            for (k, v) in &flags {
                match k.as_str() {
                    "budget" => {
                        let (name, n) = v.split_once('=').ok_or("--budget wants NAME=N")?;
                        let n: usize = n.parse().map_err(|_| "bad budget number")?;
                        if name == "total" {
                            budget.total = Some(n);
                        } else {
                            budget.groups.insert(name.to_string(), n);
                        }
                    }
                    "budget-per-group" => {
                        budget.per_group = Some(v.parse().map_err(|_| "bad budget number")?)
                    }
                    _ => {}
                }
            }
            opts.budget = budget;
            let cooked = cook(&svg, &opts).map_err(|e| e.to_string())?;
            std::fs::write(out, &cooked.bytes).map_err(|e| format!("{out}: {e}"))?;
            if flag("quiet").is_none() {
                println!("{}", cooked.report);
            }
            Ok(())
        }
        "raster" => {
            let input = positional.first().ok_or("missing input.psvg")?;
            let out = flag("o").ok_or("missing -o <out.ppm>")?;
            let bytes = std::fs::read(input).map_err(|e| format!("{input}: {e}"))?;
            let aligned = AlignedAsset::new(&bytes);
            let asset = VectorAsset::parse(aligned.bytes()).map_err(|e| format!("{e:?}"))?;
            let canvas = asset.canvas_q4();
            let size = match flag("size") {
                Some(s) => pair(s)?,
                None => vec![(canvas[0] as i64 + 15) / 16, (canvas[1] as i64 + 15) / 16],
            };
            let origin = flag("origin")
                .map(pair)
                .transpose()?
                .unwrap_or_else(|| vec![0, 0]);
            let bg = flag("bg").unwrap_or("000000");
            let bgv = u32::from_str_radix(bg, 16).map_err(|_| "bad --bg")?;
            let mut img = Image::new(
                size[0] as usize,
                size[1] as usize,
                [(bgv >> 16) as u8, (bgv >> 8) as u8, bgv as u8],
            );
            let wanted: Option<Vec<&str>> = flag("groups").map(|g| g.split(',').collect());
            for id in asset.groups() {
                if wanted
                    .as_ref()
                    .is_some_and(|w| !w.contains(&asset.group_name(id)))
                {
                    continue;
                }
                draw_group(
                    &mut img,
                    &asset,
                    id,
                    &Placement::IDENTITY,
                    &Style::DEFAULT,
                    (origin[0] as i32, origin[1] as i32),
                )
                .map_err(|e| format!("{e:?}"))?;
            }
            let scale: usize = flag("scale").map_or(Ok(1), |s| s.parse().map_err(|_| "bad --scale"))?;
            let img = if scale > 1 { img.upscaled(scale) } else { img };
            let bytes = if out.ends_with(".png") { img.to_png() } else { img.to_ppm() };
            std::fs::write(out, bytes).map_err(|e| format!("{out}: {e}"))
        }
        "diff" => {
            let (a, b) = (
                positional.first().ok_or("missing a.ppm")?,
                positional.get(1).ok_or("missing b.ppm")?,
            );
            let load = |p: &String| -> Result<Image, String> {
                Image::from_ppm(&std::fs::read(p).map_err(|e| format!("{p}: {e}"))?)
            };
            let (ia, ib) = (load(a)?, load(b)?);
            let threshold: u8 =
                flag("threshold").map_or(Ok(16), |t| t.parse().map_err(|_| "bad --threshold"))?;
            let region = match flag("region") {
                Some(r) => pair(r)?.iter().map(|&v| v as usize).collect::<Vec<_>>(),
                None => vec![0, 0, ia.width, ia.height],
            };
            let d = compare(
                &ia,
                &ib,
                threshold,
                [region[0], region[1], region[2], region[3]],
            );
            println!(
                "{} of {} pixels differ by more than {threshold} ({:.2}%), max channel diff {}, mean abs {:.3}",
                d.differing,
                d.total,
                d.fraction() * 100.0,
                d.max_channel,
                d.mean_abs
            );
            Ok(())
        }
        other => Err(format!("unknown command `{other}`")),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("psx-svg-cook: {e}");
            ExitCode::FAILURE
        }
    }
}
